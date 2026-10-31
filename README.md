The `m.*` capability pack: the phone face.

What the model writes is code against `m.*`:

```python
obs = m.observe("ax")
for n in obs.nodes:
    if n.text == "Settings":
        n.click()
        break
shot = m.observe("vision")      # -> Obs with .image (ImageHandle)
```

The pack speaks only the `phi-ns` contract over a `Transport`; assembly and
policy live above it. It never writes transport code — only the
method-to-wire mapping and the target geometry.

Node actions carry a node id and a revision; a stale revision is refused
(`STALE_TARGET`), never silently re-targeted.

```toml
[dependencies]
phi-ns-mobile = "0.1.0"
```

## License

MIT.
