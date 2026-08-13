use futures_util::{SinkExt, StreamExt};
use native_protocol_project::proto::{EchoRequest, app_a_client::AppAClient, app_b_client::AppBClient};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
	let args: Vec<_> = std::env::args().collect();
	let http = &args[1];
	let grpc = &args[2];
	let client = reqwest::Client::new();
	let a = client.get(format!("http://{http}/app-a/")).send().await?.text().await?;
	let b = client.get(format!("http://{http}/app-b/")).send().await?.text().await?;

	let (mut ws, _) = connect_async(format!("ws://{http}/ws/app-a/")).await?;
	ws.send(Message::Text("ping".into())).await?;
	let ws_reply = ws.next().await.ok_or("WebSocket closed")??;

	let mut grpc_a = AppAClient::connect(format!("http://{grpc}")).await?;
	let mut grpc_b = AppBClient::connect(format!("http://{grpc}")).await?;
	let a_reply = grpc_a.echo(EchoRequest { message: "ping".into() }).await?.into_inner().message;
	let b_reply = grpc_b.echo(EchoRequest { message: "ping".into() }).await?.into_inner().message;

	println!("HTTP_A={a};HTTP_B={b};WS={ws_reply:?};GRPC_A={a_reply};GRPC_B={b_reply}");
	Ok(())
}
