use axum::{
    http::{HeaderMap, HeaderValue, header::CONTENT_TYPE},
    response::Html as AxumHtml,
};
use std::sync::LazyLock;

#[cfg(debug_assertions)]
type Html = AxumHtml<String>;
#[cfg(not(debug_assertions))]
type Html = AxumHtml<&'static str>;

#[cfg(debug_assertions)]
pub async fn index() -> Html {
    #[allow(clippy::unwrap_used)]
    AxumHtml(
        tokio::fs::read_to_string("frontend/index.html")
            .await
            .unwrap(),
    )
}

#[cfg(not(debug_assertions))]
pub async fn index() -> Html {
    AxumHtml(include_str!("../frontend/index.html"))
}

#[cfg(debug_assertions)]
pub async fn serve_schema() -> (HeaderMap, String) {
    static SCHEMA: LazyLock<(HeaderMap, String)> = LazyLock::new(|| {
        let string =
            std::fs::read_to_string("frontend/schema.json").expect("failed to load schema");
        let mut map = HeaderMap::new();
        map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        (map, string)
    });
    SCHEMA.clone()
}

#[cfg(not(debug_assertions))]
pub async fn serve_schema() -> (HeaderMap, &'static str) {
    static SCHEMA: LazyLock<(HeaderMap, &'static str)> = LazyLock::new(|| {
        let mut map = HeaderMap::new();
        map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        (map, include_str!("../frontend/schema.json"))
    });
    SCHEMA.clone()
}
