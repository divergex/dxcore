#[cfg(feature = "integration")]
mod integration {
    use dxcore::interface::external::guardian;
    use std::env;

    fn api_key() -> String {
        env::var("GUARDIAN_API_KEY").unwrap_or_else(|_| "test".into())
    }

    #[test]
    fn search_returns_articles() {
        let articles = guardian::search("markets", &api_key(), 5).expect("search failed");

        assert!(!articles.is_empty(), "no articles returned");
        for a in &articles {
            assert!(!a.web_title.is_empty(), "web_title is empty");
            assert!(a.web_url.starts_with("https://www.theguardian.com/"), "web_url malformed: {}", a.web_url);
        }
    }

    #[test]
    fn get_article_returns_body() {
        let articles = guardian::search("markets", &api_key(), 1).expect("search failed");
        let article = &articles[0];

        let body = guardian::get_article(&article.id, &api_key()).expect("get_article failed");

        assert_eq!(body.id, article.id);
        assert!(!body.web_title.is_empty());

        // Live blogs may not have standard body blocks
        if let Some(blocks) = &body.blocks {
            if let Some(body_blocks) = &blocks.body {
                assert!(!body_blocks.is_empty(), "body blocks are empty");
                assert!(!body_blocks[0].body_html.is_empty());
            }
        }
    }
}
