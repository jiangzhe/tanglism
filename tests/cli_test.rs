use mockito::mock;
use jqdata::*;

#[test]
fn test_get_all_securities() {
    let response_body = {
        let mut s = String::from("code,display_name,name,start_date,end_date,type\n");
        s.push_str("000001.XSHE,平安银行,PAYH,1991-04-03,2200-01-01,stock\n");
        s.push_str("000002.XSHE,万科A,WKA,1991-01-29,2200-01-01,stock\n");
        s
    };
    let _m = mock("POST", "/")
        .with_status(200)
        .with_body(&response_body)
        .create();
    
    let client = JqdataClient::with_token("abc").unwrap();
    let ss = client.execute(GetAllSecurities{code: SecurityKind::Stock, date: None}).unwrap();
    assert_eq!(vec![
        Security{
            code: "000001.XSHE".to_string(),
            display_name: "平安银行".to_string(),
            name: "PAYH".to_string(),
            start_date: "1991-04-03".to_string(),
            end_date: "2200-01-01".to_string(),
            kind: SecurityKind::Stock,
            parent: None,
        },
        Security{
            code: "000002.XSHE".to_string(),
            display_name: "万科A".to_string(),
            name: "WKA".to_string(),
            start_date: "1991-01-29".to_string(),
            end_date: "2200-01-01".to_string(),
            kind: SecurityKind::Stock,
            parent: None,
        }
    ], ss);


}