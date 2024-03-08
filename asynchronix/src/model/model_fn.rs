//! Trait for model input and replier ports.

use std::future::{ready, Future, Ready};

use crate::model::{markers, Model};
use crate::time::Scheduler;

/// A function, method or closures that can be used as an *input port*.
///
/// This trait is in particular implemented for any function or method with the
/// following signature, where it is implicitly assumed that the function
/// implements `Send + 'static`:
///
/// ```ignore
/// FnOnce(&mut M, T)
/// FnOnce(&mut M, T, &Scheduler<M>)
/// async fn(&mut M, T)
/// async fn(&mut M, T, &Scheduler<M>)
/// where
///     M: Model
/// ```
///
/// It is also implemented for the following signatures when `T=()`:
///
/// ```ignore
/// FnOnce(&mut M)
/// async fn(&mut M)
/// where
///     M: Model
/// ```
pub trait InputFn<'a, M: Model, T, S>: Send + 'static {
    /// The `Future` returned by the asynchronous method.
    type Future: Future<Output = ()> + Send + 'a;

    /// Calls the method.
    fn call(self, model: &'a mut M, arg: T, scheduler: &'a Scheduler<M>) -> Self::Future;
}

impl<'a, M, F> InputFn<'a, M, (), markers::WithoutArguments> for F
where
    M: Model,
    F: FnOnce(&'a mut M) + Send + 'static,
{
    type Future = Ready<()>;

    fn call(self, model: &'a mut M, _arg: (), _scheduler: &'a Scheduler<M>) -> Self::Future {
        self(model);

        ready(())
    }
}

impl<'a, M, T, F> InputFn<'a, M, T, markers::WithoutScheduler> for F
where
    M: Model,
    F: FnOnce(&'a mut M, T) + Send + 'static,
{
    type Future = Ready<()>;

    fn call(self, model: &'a mut M, arg: T, _scheduler: &'a Scheduler<M>) -> Self::Future {
        self(model, arg);

        ready(())
    }
}

impl<'a, M, T, F> InputFn<'a, M, T, markers::WithScheduler> for F
where
    M: Model,
    F: FnOnce(&'a mut M, T, &'a Scheduler<M>) + Send + 'static,
{
    type Future = Ready<()>;

    fn call(self, model: &'a mut M, arg: T, scheduler: &'a Scheduler<M>) -> Self::Future {
        self(model, arg, scheduler);

        ready(())
    }
}

impl<'a, M, Fut, F> InputFn<'a, M, (), markers::AsyncWithoutArguments> for F
where
    M: Model,
    Fut: Future<Output = ()> + Send + 'a,
    F: FnOnce(&'a mut M) -> Fut + Send + 'static,
{
    type Future = Fut;

    fn call(self, model: &'a mut M, _arg: (), _scheduler: &'a Scheduler<M>) -> Self::Future {
        self(model)
    }
}

impl<'a, M, T, Fut, F> InputFn<'a, M, T, markers::AsyncWithoutScheduler> for F
where
    M: Model,
    Fut: Future<Output = ()> + Send + 'a,
    F: FnOnce(&'a mut M, T) -> Fut + Send + 'static,
{
    type Future = Fut;

    fn call(self, model: &'a mut M, arg: T, _scheduler: &'a Scheduler<M>) -> Self::Future {
        self(model, arg)
    }
}

impl<'a, M, T, Fut, F> InputFn<'a, M, T, markers::AsyncWithScheduler> for F
where
    M: Model,
    Fut: Future<Output = ()> + Send + 'a,
    F: FnOnce(&'a mut M, T, &'a Scheduler<M>) -> Fut + Send + 'static,
{
    type Future = Fut;

    fn call(self, model: &'a mut M, arg: T, scheduler: &'a Scheduler<M>) -> Self::Future {
        self(model, arg, scheduler)
    }
}

/// A function, method or closure that can be used as a *replier port*.
///
/// This trait is in particular implemented for any function or method with the
/// following signature, where it is implicitly assumed that the function
/// implements `Send + 'static`:
///
/// ```ignore
/// async fn(&mut M, T) -> R
/// async fn(&mut M, T, &Scheduler<M>) -> R
/// where
///     M: Model
/// ```
///
/// It is also implemented for the following signatures when `T=()`:
///
/// ```ignore
/// async fn(&mut M) -> R
/// where
///     M: Model
/// ```
pub trait ReplierFn<'a, M: Model, T, R, S>: Send + 'static {
    /// The `Future` returned by the asynchronous method.
    type Future: Future<Output = R> + Send + 'a;

    /// Calls the method.
    fn call(self, model: &'a mut M, arg: T, scheduler: &'a Scheduler<M>) -> Self::Future;
}

impl<'a, M, R, Fut, F> ReplierFn<'a, M, (), R, markers::AsyncWithoutArguments> for F
where
    M: Model,
    Fut: Future<Output = R> + Send + 'a,
    F: FnOnce(&'a mut M) -> Fut + Send + 'static,
{
    type Future = Fut;

    fn call(self, model: &'a mut M, _arg: (), _scheduler: &'a Scheduler<M>) -> Self::Future {
        self(model)
    }
}

impl<'a, M, T, R, Fut, F> ReplierFn<'a, M, T, R, markers::AsyncWithoutScheduler> for F
where
    M: Model,
    Fut: Future<Output = R> + Send + 'a,
    F: FnOnce(&'a mut M, T) -> Fut + Send + 'static,
{
    type Future = Fut;

    fn call(self, model: &'a mut M, arg: T, _scheduler: &'a Scheduler<M>) -> Self::Future {
        self(model, arg)
    }
}

impl<'a, M, T, R, Fut, F> ReplierFn<'a, M, T, R, markers::AsyncWithScheduler> for F
where
    M: Model,
    Fut: Future<Output = R> + Send + 'a,
    F: FnOnce(&'a mut M, T, &'a Scheduler<M>) -> Fut + Send + 'static,
{
    type Future = Fut;

    fn call(self, model: &'a mut M, arg: T, scheduler: &'a Scheduler<M>) -> Self::Future {
        self(model, arg, scheduler)
    }
}

#[cfg(test)]
mod tests {
    use futures_util::Future;

    use crate::{
        model::{markers, Model},
        time::Scheduler,
    };

    trait InputFnTest<'a, M: Model, T, S>: Send + 'static {
        /// The `Future` returned by the asynchronous method.
        type Future: Future<Output = ()> + Send + 'a;
        type Args;

        /// Calls the method.
        fn call(
            self,
            model: &'a mut M,
            arg: Self::Args,
            scheduler: &'a Scheduler<M>,
        ) -> Self::Future;
    }

    impl<'a, M, T, Fut, F> InputFnTest<'a, M, fn(T), markers::AsyncWithScheduler> for F
    where
        M: Model,
        Fut: Future<Output = ()> + Send + 'a,
        F: FnOnce(&'a mut M, T, &'a Scheduler<M>) -> Fut + Send + 'static,
    {
        type Future = Fut;
        type Args = T;

        fn call(
            self,
            model: &'a mut M,
            arg: Self::Args,
            scheduler: &'a Scheduler<M>,
        ) -> Self::Future {
            self(model, arg, scheduler)
        }
    }

    impl<'a, M, T0, T1, Fut, F> InputFnTest<'a, M, fn(T0, T1), markers::AsyncWithScheduler> for F
    where
        M: Model,
        Fut: Future<Output = ()> + Send + 'a,
        F: FnOnce(&'a mut M, T0, T1, &'a Scheduler<M>) -> Fut + Send + 'static,
    {
        type Future = Fut;
        type Args = (T0, T1);

        fn call(
            self,
            model: &'a mut M,
            args: Self::Args,
            scheduler: &'a Scheduler<M>,
        ) -> Self::Future {
            let (arg0, arg1) = args;
            self(model, arg0, arg1, scheduler)
        }
    }

    struct TestModel {
    }

    impl TestModel {
        async fn input_fn0(&mut self, arg0: u32, _: &Scheduler<Self>) {}
        async fn input_fn1(&mut self, arg0: u32, arg1: i32, _: &Scheduler<Self>) {}
    }

    impl Model for TestModel {}

    fn test_input_fn_impl_0<T, F: for<'a> InputFnTest<'a, TestModel, fn(T), markers::AsyncWithScheduler>>(func: F) {}
    fn test_input_fn_impl_1<T0, T1, F: for<'a> InputFnTest<'a, TestModel, fn(T0, T1), markers::AsyncWithScheduler>>(func: F) {}

    #[test]
    fn test_trait_impls() {
        let test = TestModel {};
        test_input_fn_impl_0(TestModel::input_fn0);
        test_input_fn_impl_1(TestModel::input_fn1);
    }
}
