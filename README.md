
For no cert, do the following:
```bash
cargo build --release

./target/release/timeline_check -u username -p password --hosts path-to-file-with-hosts no-cert
```

Else:
```bash
cargo build --release

./target/release/timeline_check -u username -p password --hosts path-to-file-with-hosts cert --root-cert path-to-ca --client-cert path-to-client-cert --client-key path-to-you-get-it 
```

TODO: Error handling orz
