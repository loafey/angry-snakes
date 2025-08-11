use axum::response::Html as AxumHtml;

type Html = AxumHtml<String>;

pub async fn index() -> Html {
    #[allow(clippy::unwrap_used)]
    AxumHtml(
        tokio::fs::read_to_string("frontend/index.html")
            .await
            .unwrap(),
    )
}
