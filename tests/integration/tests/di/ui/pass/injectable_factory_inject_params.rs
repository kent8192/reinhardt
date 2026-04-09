use reinhardt_di::{Depends, DiResult, Injectable, InjectionContext, injectable_factory};

#[derive(Clone, Debug)]
struct MyService {
    value: String,
}

#[async_trait::async_trait]
impl Injectable for MyService {
    async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
        Ok(Self {
            value: "test".to_string(),
        })
    }
}

#[derive(Clone, Debug)]
struct MyConfig {
    host: String,
}

#[async_trait::async_trait]
impl Injectable for MyConfig {
    async fn inject(_ctx: &InjectionContext) -> DiResult<Self> {
        Ok(Self {
            host: "localhost".to_string(),
        })
    }
}

// Case 1: #[inject] with non-Depends type (requires Clone)
#[injectable_factory(scope = "transient")]
async fn make_handler(#[inject] service: MyService) -> String {
    service.value
}

// Case 2: #[inject] with Depends<T> type
#[injectable_factory(scope = "transient")]
async fn make_router(#[inject] config: Depends<MyConfig>) -> String {
    config.host.clone()
}

// Case 3: Multiple #[inject] parameters mixing both patterns
#[injectable_factory(scope = "transient")]
async fn make_app(
    #[inject] service: MyService,
    #[inject] config: Depends<MyConfig>,
) -> String {
    format!("{}:{}", config.host, service.value)
}

fn main() {}
