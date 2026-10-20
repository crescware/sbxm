/// fixtureのdaemon socketとruntime IDを指定する、secretを保持した削除要求。
pub fn removal_command(name: &str, force: bool) -> String {
    format!(
        "--disable --silent --show-error --fail-with-body --noproxy * --unix-socket /tmp/sbxm-test-sandboxd.sock --request DELETE http://localhost/sandbox/{name}?force={force}&expected_id=ec55cefe-9919-4c0e-952c-db88e5466db2&delete_scoped_secrets=false"
    )
}
