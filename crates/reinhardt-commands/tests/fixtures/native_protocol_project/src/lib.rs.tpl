pub mod proto {
	tonic::include_proto!("native.protocol");
}

use reinhardt::{ConsumerContext, Message, Response, UnifiedRouter, ViewResult, WebSocketResult};
use reinhardt::{get, routes, websocket};

#[get("/app-a/", name = "app-a")]
async fn app_a_http() -> ViewResult<Response> {
	Ok(Response::ok().with_body(b"app-a".to_vec()))
}

#[get("/app-b/", name = "app-b")]
async fn app_b_http() -> ViewResult<Response> {
	Ok(Response::ok().with_body(b"app-b".to_vec()))
}

#[websocket("/ws/app-a/", name = "app-a-ws")]
async fn app_a_ws(context: &mut ConsumerContext, message: Message) -> WebSocketResult<()> {
	if let Message::Text { data } = message {
		context.connection.send_text(format!("app-a:{data}")).await?;
	}
	Ok(())
}

#[websocket("/ws/app-b/", name = "app-b-ws")]
async fn app_b_ws(context: &mut ConsumerContext, message: Message) -> WebSocketResult<()> {
	if let Message::Text { data } = message {
		context.connection.send_text(format!("app-b:{data}")).await?;
	}
	Ok(())
}

#[derive(Default)]
pub struct AppAService;

#[tonic::async_trait]
impl proto::app_a_server::AppA for AppAService {
	async fn echo(
		&self,
		request: tonic::Request<proto::EchoRequest>,
	) -> Result<tonic::Response<proto::EchoReply>, tonic::Status> {
		Ok(tonic::Response::new(proto::EchoReply {
			message: format!("app-a:{}", request.into_inner().message),
		}))
	}
}

#[derive(Default)]
pub struct AppBService;

#[tonic::async_trait]
impl proto::app_b_server::AppB for AppBService {
	async fn echo(
		&self,
		request: tonic::Request<proto::EchoRequest>,
	) -> Result<tonic::Response<proto::EchoReply>, tonic::Status> {
		Ok(tonic::Response::new(proto::EchoReply {
			message: format!("app-b:{}", request.into_inner().message),
		}))
	}
}

pub fn app_a_routes() -> UnifiedRouter {
	UnifiedRouter::new()
		.server(|router| router.endpoint(app_a_http))
		.websocket(|router| router.consumer(app_a_ws))
		.grpc(|router| {
			router.service(proto::app_a_server::AppAServer::new(AppAService))
		})
}

pub fn app_b_routes() -> UnifiedRouter {
	UnifiedRouter::new()
		.server(|router| router.endpoint(app_b_http))
		.websocket(|router| router.consumer(app_b_ws))
		.grpc(|router| {
			router.service(proto::app_b_server::AppBServer::new(AppBService))
		})
}

#[routes]
pub fn routes() -> UnifiedRouter {
	UnifiedRouter::new()
		.merge(app_a_routes())
		.merge(app_b_routes())
}
