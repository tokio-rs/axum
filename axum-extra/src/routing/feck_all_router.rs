#[cfg(feature = "tokio")]
use crate::extract::connect_info::IntoMakeServiceWithConnectInfo;
use axum::{body::{Body, Bytes, HttpBody},    
    error_handling::{HandleError, HandleErrorLayer},
    handler::Handler,
    http::{Method, Request, StatusCode},
    response::{Response, IntoResponse},
    routing::{future::RouteFuture, Route, Fallback}, RouteResolver,
    };

use bytes::BytesMut;
use std::{
    convert::Infallible,
    fmt,
    task::{Context, Poll}, marker::PhantomData,
};
use tower::{service_fn, util::MapResponseLayer};
use tower_layer::Layer;
use tower_service::Service;
use axum::routing::BoxedIntoRoute;





pub struct FeckAllRouter <S, B = Body, E = Infallible> where 
    B: HttpBody + Send+ 'static,
    S: Clone + 'static + Send + Sync + Default,        
    E: From<Infallible> + 'static
    {        
        handler : Option<BoxedIntoRoute<S, B, E>>,                
        state : S,
    }




impl<S, B, E> Clone for FeckAllRouter<S, B, E> where     
    B: HttpBody + Send+ 'static,
    S: Default + Clone + Send + Sync ,
    E: From<Infallible> + 'static
{
    fn clone(&self) -> Self {
        Self {
            handler: self.handler.clone(),            
            state: self.state.clone(),            
        }
    }

}


impl<S, B> FeckAllRouter<S, B> where     
B: HttpBody + Send+ 'static,
S: Default + Clone + Send + Sync ,
{
    pub fn new()->Self{
        
        FeckAllRouter{
            handler : None,
            state: Default::default(),              
        }
    }
}

impl<S, B> FeckAllRouter<S, B, Infallible> where
    B: HttpBody + Send+  'static,
    S: Clone + 'static +  Send + Sync + Default,                
{
    #[track_caller]
    pub fn on<H,T> (self, handler: H) -> Self 
    where
        H: Handler<T, S, B>,
        T: 'static,
        S: Send + Sync + 'static,    
    {
        FeckAllRouter{
            handler : Some(BoxedIntoRoute::from_handler(handler)),
            state: self.state.clone(),              
        }
        
    }
}

impl<S, B, E> RouteResolver<S,B,E> for FeckAllRouter<S, B, E> where 
    S: Clone + 'static + Default + Sync + Send,
    B: HttpBody + Send+  'static ,        
    E: From<Infallible> + 'static
    {
 
    fn with_state(self, state: S) -> Self{
        FeckAllRouter{
            handler: self.handler.clone(),            
            state : self.state.clone(),
        }
    }    

    
    fn layer<L>(self, layer: L) -> FeckAllRouter<S, B, E> 
    where
    L: Layer<Route<B, E>> + Clone + Send + 'static,
    L::Service: Service<Request<B>> + Clone + Send + 'static,
    <L::Service as Service<Request<B>>>::Response: IntoResponse + 'static,
    <L::Service as Service<Request<B>>>::Error: Into<E> + 'static,
    <L::Service as Service<Request<B>>>::Future: Send + 'static,                                
    {
        FeckAllRouter{
            handler: self.handler.clone(),            
            state : self.state.clone(),
        }
    }
    
    #[track_caller]
    fn route_layer<L>(mut self, layer: L) -> Self
    where    
        L: Layer<Route<B, E>> + Clone + Send + 'static,
        L::Service: Service<Request<B>, Error = E> + Clone + Send + 'static,
        <L::Service as Service<Request<B>>>::Response: IntoResponse + 'static,
        <L::Service as Service<Request<B>>>::Future: Send + 'static,        
    {        
        FeckAllRouter{
            handler: self.handler.clone(),            
            state : self.state.clone(),
        }
    }

    #[track_caller]
    fn merge_for_path(
        mut self,
        path: Option<&str>,
        other: FeckAllRouter<S, B, E>,
    ) -> Self {
        // written using inner functions to generate less IR        
        FeckAllRouter{
            handler: self.handler.clone(),            
            state : self.state.clone(),
        }
    }

    
    fn call_with_state(&mut self, req: Request<B>, state: S) -> RouteFuture<B, E> { 
        if let Some(ref handler)  = self.handler{
            let mut route = handler.clone().into_route(state);
            RouteFuture::from_future(route.oneshot_inner(req))        
        }else{
            let mut route = Route::new(service_fn(|_: Request<B>| async {
                Ok(StatusCode::METHOD_NOT_ALLOWED.into_response())
            }));            
            RouteFuture::from_future(route.oneshot_inner(req))
            
        }
    }

    fn call_with_no_state(&mut self, req: Request<B>) -> RouteFuture<B, E> {
        if let Some(ref handler)  = self.handler{
            let mut route = handler.clone().into_route(self.state.clone());
            RouteFuture::from_future(route.oneshot_inner(req))        
        }else{
            let mut route = Route::new(service_fn(|_: Request<B>| async {
                Ok(StatusCode::METHOD_NOT_ALLOWED.into_response())
            }));            
            RouteFuture::from_future(route.oneshot_inner(req))
            
        }
        
    }
}
