macro_rules! test_filename {
    ($format:expr) => {{
        let date = chrono::Utc::now().to_rfc3339().replace(":", "-");
        format!("/tmp/sinowealth-kb-tool-{}-{}.{}", date, stdext::function_name!().replace(":", "_").replace("::{{closure}}", ""), $format)
    }};
}
