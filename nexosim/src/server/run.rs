//! Simulation server.

use std::net::SocketAddr;
#[cfg(unix)]
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;

use serde::de::DeserializeOwned;
use tonic::{transport::Server, Request, Response, Status};

use crate::registry::EndpointRegistry;
use crate::simulation::{Simulation, SimulationError};

use super::codegen::simulation::*;
use super::key_registry::KeyRegistry;
use super::services::InitService;
use super::services::{ControllerService, MonitorService, SchedulerService};

/// Runs a simulation from a network server.
///
/// The first argument is a closure that takes an initialization configuration
/// and is called every time the simulation is (re)started by the remote client.
/// It must create a new simulation, complemented by a registry that exposes the
/// public event and query interface.
pub fn run<F, I>(sim_gen: F, addr: SocketAddr) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(I) -> Result<(Simulation, EndpointRegistry), SimulationError> + Send + 'static,
    I: DeserializeOwned,
{
    run_service(GrpcSimulationService::new(sim_gen), addr)
}

/// Monomorphization of the network server.
///
/// Keeping this as a separate monomorphized fragment can even triple
/// compilation speed for incremental release builds.
fn run_service(
    service: GrpcSimulationService,
    addr: SocketAddr,
) -> Result<(), Box<dyn std::error::Error>> {
    // Use 2 threads so that even if the controller service is blocked due to
    // ongoing simulation execution, other services can still be used
    // concurrently.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_io()
        .build()?;

    rt.block_on(async move {
        Server::builder()
            .add_service(simulation_server::SimulationServer::new(service))
            .serve(addr)
            .await?;

        Ok(())
    })
}

/// Runs a simulation locally from a Unix Domain Sockets server.
///
/// The first argument is a closure that takes an initialization configuration
/// and is called every time the simulation is (re)started by the remote client.
/// It must create a new simulation, complemented by a registry that exposes the
/// public event and query interface.
#[cfg(unix)]
pub fn run_local<F, I, P>(sim_gen: F, path: P) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnMut(I) -> Result<(Simulation, EndpointRegistry), SimulationError> + Send + 'static,
    I: DeserializeOwned,
    P: AsRef<Path>,
{
    let path = path.as_ref();
    run_local_service(GrpcSimulationService::new(sim_gen), path)
}

/// Monomorphization of the Unix Domain Sockets server.
///
/// Keeping this as a separate monomorphized fragment can even triple
/// compilation speed for incremental release builds.
#[cfg(unix)]
fn run_local_service(
    service: GrpcSimulationService,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs;
    use std::io;
    use std::os::unix::fs::FileTypeExt;

    use tokio::net::UnixListener;
    use tokio_stream::wrappers::UnixListenerStream;

    // Unlink the socket if it already exists to prevent an `AddrInUse` error.
    match fs::metadata(path) {
        // The path is valid: make sure it actually points to a socket.
        Ok(socket_meta) => {
            if !socket_meta.file_type().is_socket() {
                return Err(Box::new(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "the specified path points to an existing non-socket file",
                )));
            }

            fs::remove_file(path)?;
        }
        // Nothing to do: the socket does not exist yet.
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        // We don't have permission to use the socket.
        Err(e) => return Err(Box::new(e)),
    }

    // (Re-)Create the socket.
    fs::create_dir_all(path.parent().unwrap())?;

    // Use 2 threads so that even if the controller service is blocked due to
    // ongoing simulation execution, other services can still be used
    // concurrently.
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_io()
        .build()?;

    rt.block_on(async move {
        let uds = UnixListener::bind(path)?;
        let uds_stream = UnixListenerStream::new(uds);

        Server::builder()
            .add_service(simulation_server::SimulationServer::new(service))
            .serve_with_incoming(uds_stream)
            .await?;

        Ok(())
    })
}

struct GrpcSimulationService {
    init_service: Mutex<InitService>,
    controller_service: Mutex<ControllerService>,
    monitor_service: Mutex<MonitorService>,
    scheduler_service: Mutex<SchedulerService>,
}

impl GrpcSimulationService {
    /// Creates a new `GrpcSimulationService` without any active simulation.
    ///
    /// The argument is a closure that takes an initialization configuration and
    /// is called every time the simulation is (re)started by the remote client.
    /// It must create a new simulation, complemented by a registry that exposes
    /// the public event and query interface.
    pub(crate) fn new<F, I>(sim_gen: F) -> Self
    where
        F: FnMut(I) -> Result<(Simulation, EndpointRegistry), SimulationError> + Send + 'static,
        I: DeserializeOwned,
    {
        Self {
            init_service: Mutex::new(InitService::new(sim_gen)),
            controller_service: Mutex::new(ControllerService::NotStarted),
            monitor_service: Mutex::new(MonitorService::NotStarted),
            scheduler_service: Mutex::new(SchedulerService::NotStarted),
        }
    }

    /// Locks the initializer and returns the mutex guard.
    fn initializer(&self) -> MutexGuard<'_, InitService> {
        self.init_service.lock().unwrap()
    }

    /// Locks the controller and returns the mutex guard.
    fn controller(&self) -> MutexGuard<'_, ControllerService> {
        self.controller_service.lock().unwrap()
    }

    /// Locks the monitor and returns the mutex guard.
    fn monitor(&self) -> MutexGuard<'_, MonitorService> {
        self.monitor_service.lock().unwrap()
    }

    /// Locks the scheduler and returns the mutex guard.
    fn scheduler(&self) -> MutexGuard<'_, SchedulerService> {
        self.scheduler_service.lock().unwrap()
    }
}

#[tonic::async_trait]
impl simulation_server::Simulation for GrpcSimulationService {
    async fn init(&self, request: Request<InitRequest>) -> Result<Response<InitReply>, Status> {
        let request = request.into_inner();

        let (reply, bench) = self.initializer().init(request);

        if let Some((simulation, scheduler, endpoint_registry)) = bench {
            let event_source_registry = Arc::new(endpoint_registry.event_source_registry);
            let query_source_registry = endpoint_registry.query_source_registry;
            let event_sink_registry = endpoint_registry.event_sink_registry;

            *self.controller() = ControllerService::Started {
                simulation,
                event_source_registry: event_source_registry.clone(),
                query_source_registry,
            };
            *self.monitor() = MonitorService::Started {
                event_sink_registry,
            };
            *self.scheduler() = SchedulerService::Started {
                scheduler,
                event_source_registry,
                key_registry: KeyRegistry::default(),
            };
        }

        Ok(Response::new(reply))
    }
    async fn halt(&self, request: Request<HaltRequest>) -> Result<Response<HaltReply>, Status> {
        let request = request.into_inner();

        Ok(Response::new(self.scheduler().halt(request)))
    }
    async fn time(&self, request: Request<TimeRequest>) -> Result<Response<TimeReply>, Status> {
        let request = request.into_inner();

        Ok(Response::new(self.scheduler().time(request)))
    }
    async fn step(&self, request: Request<StepRequest>) -> Result<Response<StepReply>, Status> {
        let request = request.into_inner();

        Ok(Response::new(self.controller().step(request)))
    }
    async fn step_until(
        &self,
        request: Request<StepUntilRequest>,
    ) -> Result<Response<StepUntilReply>, Status> {
        let request = request.into_inner();

        Ok(Response::new(self.controller().step_until(request)))
    }
    async fn schedule_event(
        &self,
        request: Request<ScheduleEventRequest>,
    ) -> Result<Response<ScheduleEventReply>, Status> {
        let request = request.into_inner();

        Ok(Response::new(self.scheduler().schedule_event(request)))
    }
    async fn cancel_event(
        &self,
        request: Request<CancelEventRequest>,
    ) -> Result<Response<CancelEventReply>, Status> {
        let request = request.into_inner();

        Ok(Response::new(self.scheduler().cancel_event(request)))
    }
    async fn process_event(
        &self,
        request: Request<ProcessEventRequest>,
    ) -> Result<Response<ProcessEventReply>, Status> {
        let request = request.into_inner();

        Ok(Response::new(self.controller().process_event(request)))
    }
    async fn process_query(
        &self,
        request: Request<ProcessQueryRequest>,
    ) -> Result<Response<ProcessQueryReply>, Status> {
        let request = request.into_inner();

        Ok(Response::new(self.controller().process_query(request)))
    }
    async fn read_events(
        &self,
        request: Request<ReadEventsRequest>,
    ) -> Result<Response<ReadEventsReply>, Status> {
        let request = request.into_inner();

        Ok(Response::new(self.monitor().read_events(request)))
    }
    async fn open_sink(
        &self,
        request: Request<OpenSinkRequest>,
    ) -> Result<Response<OpenSinkReply>, Status> {
        let request = request.into_inner();

        Ok(Response::new(self.monitor().open_sink(request)))
    }
    async fn close_sink(
        &self,
        request: Request<CloseSinkRequest>,
    ) -> Result<Response<CloseSinkReply>, Status> {
        let request = request.into_inner();

        Ok(Response::new(self.monitor().close_sink(request)))
    }
}
