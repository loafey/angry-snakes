use std::sync::LazyLock;

use axum::{
    http::{HeaderMap, HeaderName, HeaderValue, header::CONTENT_TYPE},
    response::Html as AxumHtml,
};
use axum_extra::headers::ContentType;

type Html = AxumHtml<String>;

pub async fn index() -> Html {
    #[allow(clippy::unwrap_used)]
    AxumHtml(
        tokio::fs::read_to_string("frontend/index.html")
            .await
            .unwrap(),
    )
}

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
