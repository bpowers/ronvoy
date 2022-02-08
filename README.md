
Ronvoy - Envoy implementation in Rust
=====================================

build and start Ronvoy:

```
$ cargo build --release
$ ./target/release/ronvoy --config-path ./test/configs/upstream.yaml
```

start some upstream server on port `9110` in a second terminal:

```
$ python -m http.server 9110 --bind 127.0.0.1
```

in a separate terminal send a request to Ronvoy (which will be proxied to the python service:

```
$ curl http://localhost:9000/
```

TODO
----

- mTLS
- configuration and reconfiguration via xDS
- literally everything else