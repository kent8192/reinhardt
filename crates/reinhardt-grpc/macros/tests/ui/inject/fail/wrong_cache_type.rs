// Verify that wrong value types for `cache` option produce compile errors.

use reinhardt_grpc_macros::grpc_handler;

struct Request<T>(T);
struct Response<T>(T);
struct Status;

#[grpc_handler]
async fn handler(
	request: Request<()>,
	#[inject(cache = "yes")] service: String,
) -> Result<Response<()>, Status> {
	unimplemented!()
}

fn main() {}
