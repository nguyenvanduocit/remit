//! Sealed values already sit in the owner's database: they must keep opening. The fixture is such a value, sealed under
//! a throwaway key, the bytes 0 to 31 (never a real key).

const GO_SEALED: &str = include_str!("fixtures/stored_sealed.tsv");

fn throwaway_key() -> Vec<u8> {
    (0u8..32).collect()
}

#[test]
fn a_stored_sealed_value_opens() {
    let key = throwaway_key();
    let fields: Vec<&str> = GO_SEALED.trim_end().split('\t').collect();
    let [aad, plaintext, key_id, sealed_box] = fields[..] else {
        panic!("the fixture is aad, plaintext, key id, box: {fields:?}");
    };
    assert_eq!(secrets::key_id(&key), key_id, "the key id is not the stored one");
    let sealed = secrets::Sealed {
        key_id: key_id.to_string(),
        r#box: hex::decode(sealed_box).unwrap(),
    };
    assert_eq!(secrets::open(&key, aad, &sealed).unwrap(), plaintext.as_bytes());
    // Printed so a value sealed now can be checked by hand against the stored format.
    let rust = secrets::seal(&key, aad, plaintext.as_bytes()).unwrap();
    println!("RUST_SEALED {aad} {} {}", rust.key_id, hex::encode(&rust.r#box));
}
