{
  buildGoModule,
  fetchFromGitHub,
}: let
  name = "tanka";
  version = "unstable-2026-01-14";
in
  buildGoModule {
    inherit name version;

    src = fetchFromGitHub {
      owner = "grafana";
      repo = name;
      rev = "94c04a578a1cd49457b0b2fd00bf0c5fa74bedc5";
      hash = "sha256-ssnCsfoiwoUATkGB8UcVx2aMiBRwEZt0OeiEKKBJSak=";
    };

    vendorHash = "sha256-4U56P3MP9Sc3maGYSAP2/577IXhWpuALSd3GLJoFMcE=";

    proxyVendor = true;

    meta.mainProgram = "tk";
    subPackages = ["cmd/tk"];

    ldflags = [
      "-s"
      "-w"
      "-X github.com/grafana/tanka/pkg/tanka.CurrentVersion=v${version}"
    ];
  }
