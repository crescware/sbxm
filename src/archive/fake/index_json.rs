/// `index.json`の内容。containerd image storeの`docker image save`が書く形に倣う。
pub fn index_json(image_name: &str, index_id: &str) -> String {
    let (_, tag) = image_name
        .rsplit_once(':')
        .unwrap_or((image_name, "latest"));
    format!(
        r#"{{"schemaVersion":2,"mediaType":"application/vnd.oci.image.index.v1+json","manifests":[{{"mediaType":"application/vnd.oci.image.index.v1+json","digest":"{index_id}","size":1,"annotations":{{"io.containerd.image.name":"docker.io/library/{image_name}","org.opencontainers.image.ref.name":"{tag}"}}}}]}}"#
    )
}
