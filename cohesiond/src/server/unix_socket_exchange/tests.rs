use super::next_line;

/// Reads every line `input` holds, `limit` bytes at most each.
async fn lines(input: &[u8], limit: usize) -> Vec<String> {
    let mut read = input;
    let mut buffer = Vec::new();
    let mut lines = Vec::new();
    while let Ok(Some(line)) = next_line(&mut read, &mut buffer, limit).await {
        lines.push(line);
    }
    lines
}

#[tokio::test]
async fn a_line_longer_than_one_read_arrives_whole_and_lines_after_it_follow() {
    let long = "x".repeat(300 * 1024);
    let input = format!("{long}\n\n  \nshort\nlast\n");
    assert_eq!(
        lines(input.as_bytes(), 1024 * 1024).await,
        [long, "short".to_string(), "last".to_string()]
    );
}

#[tokio::test]
async fn a_line_past_the_limit_is_refused() {
    let input = format!("{}\n", "x".repeat(200 * 1024));
    let mut read = input.as_bytes();
    let mut buffer = Vec::new();
    assert!(next_line(&mut read, &mut buffer, 100 * 1024).await.is_err());
}
