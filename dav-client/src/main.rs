use dav_client::carddav_client::CardDavClient;

#[tokio::main]
async fn main() {
    let url = "https://muh.it-rahn.de/SOGo/dav/luca@it-rahn.de/Contacts/personal/"
        .parse()
        .unwrap();
    let client = CardDavClient::new(
        url,
        "luca@it-rahn.de",
        "{,91,nsUipwaIsTEpROTTEpoTLEneRpSTyInE",
    )
    .unwrap();
    let contacts = client.list_contacts().await.unwrap();

    let contacts = client.fetch_contacts(&contacts).await.unwrap();

    println!("{:#?}", contacts.first());
}
