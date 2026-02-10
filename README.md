<p align="center">
  <img
    width="200"
    src="./docs/rustanka.svg"
    alt="Rustanka Logo"
  />
</p>

> [!WARNING]
> Rustanka is still in its infancy, and not ready for production use. At the minute, it's hyper-optimized the the workflows that we use at Grafana Labs for our deployment orchestration pipelines, with things changing every day. In the future we hope to extend it to be more widely useful for the existing Tanka community.

# Rustanka

<img
  src="https://raw.githubusercontent.com/grafana/tanka/main/docs/img/example.png"
  width="50%"
  align="right"
/>

**The clean, concise and super flexible alternative to YAML for your
[Kubernetes](https://k8s.io) cluster- now in Rust, and faster than ever before 🚀**

- **✨ Clean**: The
  [Jsonnet language](https://jsonnet.org) expresses your apps more obviously than YAML ever did.
- **🏗️ Reusable**: Build libraries, import them anytime and even share them on GitHub!
- **📌 Concise**: Using the Kubernetes library and abstraction, you will
  never see boilerplate again!
- **🎯 Confidence**: Stop guessing and use `rtk diff` to see what exactly will happen.
- **🔭 Helm**: Vendor in, modify, and export [Helm charts reproducibly](https://tanka.dev/helm#helm-support).
