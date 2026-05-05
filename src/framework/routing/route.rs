use axum::Router;
use axum::handler::Handler;
use axum::routing::{delete, get, patch, post, put};

pub struct Route {
    router: Router,
}

impl Route {
    pub fn new() -> Self {
        Self {
            router: Router::new(),
        }
    }

    pub fn get<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.route(path, get(handler));
        self
    }

    pub fn post<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.route(path, post(handler));
        self
    }

    pub fn put<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.route(path, put(handler));
        self
    }

    pub fn patch<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.route(path, patch(handler));
        self
    }

    pub fn delete<H, T>(mut self, path: &str, handler: H) -> Self
    where
        H: Handler<T, ()>,
        T: 'static,
    {
        self.router = self.router.route(path, delete(handler));
        self
    }

    pub fn into_router(self) -> Router {
        self.router
    }
}

impl Default for Route {
    fn default() -> Self {
        Self::new()
    }
}
