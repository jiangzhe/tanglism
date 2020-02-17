use serde_json::json;

#[test]
fn json_serialize() {
    let mob = "00000000000";
    let pass = "password";
    let json = json!({
        "mob": mob,
        "pass": pass,
    });
    assert_eq!(
        r#"{"mob":"00000000000","pass":"password"}"#,
        &json.to_string()
    );
}

#[test]
fn reqwest_baidu() {
    let response = reqwest::blocking::get("https://www.baidu.com")
        .unwrap()
        .text()
        .unwrap();
    println!("{}", response);
}
