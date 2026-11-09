use super::generated_client_name;

#[test]
fn names_the_device_then_the_client_in_parentheses() {
    let device = "apollo";
    let client = "Some client";
    assert_eq!(
        generated_client_name(device, client),
        format!("{device} ({client})")
    );
}
