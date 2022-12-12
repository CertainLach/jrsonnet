# Benchmarks

There are multiple implementations of jsonnet implemented in different languages: Rust (this repo), [Go](https://github.com/google/go-jsonnet/), [Scala](https://github.com/databricks/sjsonnet), [C++](https://github.com/google/jsonnet), [Haskell](https://github.com/moleike/haskell-jsonnet).

For simplicity, I will call these implementations by the language of their implementation.

Unfortunately, I haven't managed to measure performance of Haskell implementation, because I wasn't able to build it, and there is no binaries published anywhere, so this implementation is omitted from the following benchmarks

<details>
<summary>Tested versions</summary>

Go: Jsonnet commandline interpreter (Go implementation) v0.19.1

```
Jsonnet commandline interpreter (Go implementation) v0.19.1

jsonnet {<option>} <filename>

Available options:
  -h / --help                This message
  -e / --exec                Treat filename as code
  -J / --jpath <dir>         Specify an additional library search dir
                             (right-most wins)
  -o / --output-file <file>  Write to the output file rather than stdout
  -m / --multi <dir>         Write multiple files to the directory, list files
                             on stdout
  -c / --create-output-dirs  Automatically creates all parent directories for
                             files
  -y / --yaml-stream         Write output as a YAML stream of JSON documents
  -S / --string              Expect a string, manifest as plain text
  -s / --max-stack <n>       Number of allowed stack frames
  -t / --max-trace <n>       Max length of stack trace before cropping
  --version                  Print version

Available options for specifying values of 'external' variables:
  Provide the value as a string:
  -V / --ext-str <var>[=<val>]      If <val> is omitted, get from environment
                                    var <var>
       --ext-str-file <var>=<file>  Read the string from the file
  Provide a value as Jsonnet code:
  --ext-code <var>[=<code>]         If <code> is omitted, get from environment
                                    var <var>
  --ext-code-file <var>=<file>      Read the code from the file

Available options for specifying values of 'top-level arguments':
  Provide the value as a string:
  -A / --tla-str <var>[=<val>]      If <val> is omitted, get from environment
                                    var <var>
       --tla-str-file <var>=<file>  Read the string from the file
  Provide a value as Jsonnet code:
  --tla-code <var>[=<code>]         If <code> is omitted, get from environment
                                    var <var>
  --tla-code-file <var>=<file>      Read the code from the file

Environment variables:
  JSONNET_PATH is a colon (semicolon on Windows) separated list of directories
  added in reverse order before the paths specified by --jpath (i.e. left-most
  wins). E.g. these are equivalent:
    JSONNET_PATH=a:b jsonnet -J c -J d
    JSONNET_PATH=d:c:a:b jsonnet
    jsonnet -J b -J a -J c -J d

In all cases:
  <filename> can be - (stdin)
  Multichar options are expanded e.g. -abc becomes -a -b -c.
  The -- option suppresses option processing for subsequent arguments.
  Note that since filenames and jsonnet programs can begin with -, it is
  advised to use -- if the argument is unknown, e.g. jsonnet -- "$FILENAME".
```

C++: Jsonnet commandline interpreter v0.19.1

```
Jsonnet commandline interpreter v0.19.1

jsonnet {<option>} <filename>

Available options:
  -h / --help             This message
  -e / --exec             Treat filename as code
  -J / --jpath <dir>      Specify an additional library search dir (right-most wins)
  -o / --output-file <file> Write to the output file rather than stdout
  -m / --multi <dir>      Write multiple files to the directory, list files on stdout
  -y / --yaml-stream      Write output as a YAML stream of JSON documents
  -S / --string           Expect a string, manifest as plain text
  -s / --max-stack <n>    Number of allowed stack frames
  -t / --max-trace <n>    Max length of stack trace before cropping
  --gc-min-objects <n>    Do not run garbage collector until this many
  --gc-growth-trigger <n> Run garbage collector after this amount of object growth
  --version               Print version
Available options for specifying values of 'external' variables:
Provide the value as a string:
  -V / --ext-str <var>[=<val>]     If <val> is omitted, get from environment var <var>
       --ext-str-file <var>=<file> Read the string from the file
Provide a value as Jsonnet code:
  --ext-code <var>[=<code>]    If <code> is omitted, get from environment var <var>
  --ext-code-file <var>=<file> Read the code from the file
Available options for specifying values of 'top-level arguments':
Provide the value as a string:
  -A / --tla-str <var>[=<val>]     If <val> is omitted, get from environment var <var>
       --tla-str-file <var>=<file> Read the string from the file
Provide a value as Jsonnet code:
  --tla-code <var>[=<code>]    If <code> is omitted, get from environment var <var>
  --tla-code-file <var>=<file> Read the code from the file
Environment variables:
JSONNET_PATH is a colon (semicolon on Windows) separated list of directories added
in reverse order before the paths specified by --jpath (i.e. left-most wins)
E.g. JSONNET_PATH=a:b jsonnet -J c -J d is equivalent to:
JSONNET_PATH=d:c:a:b jsonnet
jsonnet -J b -J a -J c -J d

In all cases:
<filename> can be - (stdin)
Multichar options are expanded e.g. -abc becomes -a -b -c.
The -- option suppresses option processing for subsequent arguments.
Note that since filenames and jsonnet programs can begin with -, it is advised to
use -- if the argument is unknown, e.g. jsonnet -- "$FILENAME".
```

Scala:

```
Missing argument: file <str>
Expected Signature: Sjsonnet 0.4.3
usage: sjsonnet [sjsonnet-options] script-file
  -i --interactive         Run Mill in interactive mode, suitable for opening REPLs and taking user
                           input
  -J --jpath <str>         Specify an additional library search dir (right-most wins)
  -o --output-file <str>   Write to the output file rather than stdout
  -m --multi <str>         Write multiple files to the directory, list files on stdout
  -c --create-output-dirs  Automatically creates all parent directories for files
  -y --yaml-stream         Write output as a YAML stream of JSON documents
  -S --string              Expect a string, manifest as plain text
  -V --ext-str <str>       <var>[=<val>] Provide 'external' variable as string. 'If <val> is
                           omitted, get from environment var <var>
  --ext-str-file <str>     <var>=<file> Provide 'external' variable as string from the file
  -V --ext-code <str>      <var>[=<code>] Provide 'external' variable as Jsonnet code. If <code> is
                           omitted, get from environment var <var>
  --ext-code-file <str>    <var>=<file> Provide 'external' variable as Jsonnet code from the file
  -A --tla-str <str>       <var>[=<val>] Provide top-level arguments as string. 'If <val> is
                           omitted, get from environment var <var>
  --tla-str-file <str>     <var>=<file> Provide top-level arguments variable as string from the file
  -V --tla-code <str>      <var>[=<val>] Provide top-level arguments as Jsonnet code. 'If <val> is
                           omitted, get from environment var <var>
  --tla-code-file <str>    <var>=<file> Provide top-level arguments variable as Jsonnet code from
                           the file
  -n --indent <int>        How much to indent your output JSON
  -p --preserve-order      Preserves order of keys in the resulting JSON
  --strict                 Enforce some additional syntax limitations
  --yaml-out               Write output as a YAML document
  file <str>               The jsonnet file you wish to evaluate
  --yaml-debug             Generate source line comments in the output YAML doc to make it easier to
                           figure out where values come from.
  --no-static-errors       Turn static errors into warnings
  --fatal-warnings         Fail if any warnings were emitted


```

</details>


## Real world

### Graalvm CI

> Note: No results for C++, takes longer than a hour

<details>
<summary>Source</summary>

```jsonnet
# Common
local common = import 'common.jsonnet';
local graal_common = import 'graal-common.json';

# Compiler
local compiler = import 'compiler/ci/ci.jsonnet';

# GraalWasm
local wasm = import 'wasm/ci/ci.jsonnet';

# Espresso
local espresso = import 'espresso/ci/ci.jsonnet';

# Regex
local regex = import 'regex/ci/ci.jsonnet';

# SDK
local sdk = import 'sdk/ci/ci.jsonnet';

# SubstrateVM
local substratevm = import 'substratevm/ci/ci.jsonnet';

# Sulong
local sulong = import 'sulong/ci/ci.jsonnet';

# Tools
local tools = import 'tools/ci/ci.jsonnet';

# Truffle
local truffle = import 'truffle/ci/ci.jsonnet';

# JavaDoc
local javadoc = import "ci_includes/publish-javadoc.jsonnet";

# VM
local vm = import 'vm/ci/ci_includes/vm.jsonnet';

local verify_ci = (import 'ci-check.libsonnet').verify_ci;

{
  # Ensure that entries in common.jsonnet can be resolved.
  _checkCommon: (import 'common.jsonnet'),
  ci_resources:: (import 'ci-resources.libsonnet'),
  overlay: graal_common.ci.overlay,
  specVersion: "3",
  builds: [common.add_excludes_guard(b) for b in (
    compiler.builds +
    wasm.builds +
    espresso.builds +
    regex.builds +
    sdk.builds +
    substratevm.builds +
    sulong.builds +
    tools.builds +
    truffle.builds +
    javadoc.builds +
    vm.builds
  )],
  assert verify_ci(self.builds),
  // verify that the run-spec demo works
  assert (import "ci/ci_common/run-spec-demo.jsonnet").check(),
}

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 122.0 ± 1.9 | 118.3 | 125.8 | 1.00 |
| `Go` | 1402.0 ± 31.8 | 1359.7 | 1480.9 | 11.49 ± 0.32 |
| `Scala` | 869.3 ± 9.6 | 852.0 | 889.4 | 7.12 ± 0.14 |

### Kube-prometheus manifests

<details>
<summary>Source</summary>

```jsonnet
local kp =
  (import 'kube-prometheus/main.libsonnet') +
  // Uncomment the following imports to enable its patches
  // (import 'kube-prometheus/addons/anti-affinity.libsonnet') +
  // (import 'kube-prometheus/addons/managed-cluster.libsonnet') +
  // (import 'kube-prometheus/addons/node-ports.libsonnet') +
  // (import 'kube-prometheus/addons/static-etcd.libsonnet') +
  // (import 'kube-prometheus/addons/custom-metrics.libsonnet') +
  // (import 'kube-prometheus/addons/external-metrics.libsonnet') +
  // (import 'kube-prometheus/addons/pyrra.libsonnet') +
  {
    values+:: {
      common+: {
        namespace: 'monitoring',
      },
    },
  };

{ 'setup/0namespace-namespace': kp.kubePrometheus.namespace } +
{
  ['setup/prometheus-operator-' + name]: kp.prometheusOperator[name]
  for name in std.filter((function(name) name != 'serviceMonitor' && name != 'prometheusRule'), std.objectFields(kp.prometheusOperator))
} +
// { 'setup/pyrra-slo-CustomResourceDefinition': kp.pyrra.crd } +
// serviceMonitor and prometheusRule are separated so that they can be created after the CRDs are ready
{ 'prometheus-operator-serviceMonitor': kp.prometheusOperator.serviceMonitor } +
{ 'prometheus-operator-prometheusRule': kp.prometheusOperator.prometheusRule } +
{ 'kube-prometheus-prometheusRule': kp.kubePrometheus.prometheusRule } +
{ ['alertmanager-' + name]: kp.alertmanager[name] for name in std.objectFields(kp.alertmanager) } +
{ ['blackbox-exporter-' + name]: kp.blackboxExporter[name] for name in std.objectFields(kp.blackboxExporter) } +
{ ['grafana-' + name]: kp.grafana[name] for name in std.objectFields(kp.grafana) } +
// { ['pyrra-' + name]: kp.pyrra[name] for name in std.objectFields(kp.pyrra) if name != 'crd' } +
{ ['kube-state-metrics-' + name]: kp.kubeStateMetrics[name] for name in std.objectFields(kp.kubeStateMetrics) } +
{ ['kubernetes-' + name]: kp.kubernetesControlPlane[name] for name in std.objectFields(kp.kubernetesControlPlane) }
{ ['node-exporter-' + name]: kp.nodeExporter[name] for name in std.objectFields(kp.nodeExporter) } +
{ ['prometheus-' + name]: kp.prometheus[name] for name in std.objectFields(kp.prometheus) } +
{ ['prometheus-adapter-' + name]: kp.prometheusAdapter[name] for name in std.objectFields(kp.prometheusAdapter) }

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 113.2 ± 2.9 | 109.7 | 120.1 | 1.00 |
| `Go` | 2192.3 ± 25.1 | 2166.1 | 2262.2 | 19.36 ± 0.54 |
| `Scala` | 1117.5 ± 21.9 | 1075.8 | 1152.1 | 9.87 ± 0.31 |
| `C++` | 88573.2 ± 4833.2 | 84345.6 | 97409.2 | 782.15 ± 47.02 |

## Benchmarks from C++ jsonnet (/perf_tests)

### Large string join

<details>
<summary>Source</summary>

```jsonnet
{
    text: std.join(',', std.makeArray(76846, function(_) 'x')),
}

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 7.4 ± 0.2 | 6.9 | 8.9 | 1.00 |
| `Go` | 117.4 ± 5.1 | 112.1 | 139.2 | 15.87 ± 0.85 |
| `Scala` | 373.1 ± 4.5 | 367.8 | 383.0 | 50.45 ± 1.70 |
| `C++` | 85.9 ± 2.1 | 82.1 | 90.1 | 11.62 ± 0.46 |

### Large string template

> Note: No results for Go, fails with os stack size exhausion

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 8.2 ± 0.3 | 7.3 | 9.4 | 1.00 |
| `Scala` | 423.8 ± 5.5 | 416.2 | 437.5 | 51.62 ± 2.00 |
| `C++` | 15292.1 ± 204.0 | 15070.1 | 15840.7 | 1862.69 ± 72.39 |

### Realistic 1

<details>
<summary>Source</summary>

```jsonnet
local utils = {
  Func3(aaaaaaa, bbbbbbb, cccc)::
    'XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX/%s/XXXXX/%s/XXXXXXXXXXXXXXX/%s'
    % [aaaaaaa, bbbbbbb, cccc],
};


local long_list = [
  '001xxxxxxxxxxxxxxx-xxx',
  '002xxxxxxxxxxxxxxxxxx-xxx',
  '003xxxxxxxxxxxxxxxx-xxxxxxx',
  '004xxxxxx-xxxxxx',
  '005xxxx-xxx-xxxx',
  '006xxx-xxx-xxxxxx',
  '007xxx-xxx-xxxx-xxxx',
  '008xxx-xxx-xxxx-xxxx-xxxxxxx',
  '009xxx-xxx-xxxxx-xxxx-xxxxxxx-xxxxxx',
  '010xxx-xxx-xxxx-xxxxxxx-xxxxxx-xxxxxxx',
  '011xxx-xxx-xxxxx-xxxxxxxx-xxxxxxx',
  '012xxx-xxx-xxxx-xxxxxxxx-xxxxxxx-xxxxxx',
  '013xxx-xxx-xxxx-xxxxxxxxxx',
  '014xxx-xxx-xxxxx-xxxxxx-xx',
  '015xxx-xxx-xxxxxx-xxxxxxx',
  '016xxx-xxx-xxxx-xxxxxxxx-xxxxx',
  '017xxx-xxx-xxxx-xxxxxxxxxxxxxxx',
  '018xxx-xxx-xxxx-xxxxxxxx-xxxxxxx-xxxxxxx',
  '019xxx-xxx-xxxx-xxxxxxxx-xx',
  '020xxx-xxx-xxxx-xxxxxxxxxxxxxxxx',
  '021xxx-xxx-xxxx-xxxxxxxx-xxxxxxxxxxxx-xxx',
  '022xxx-xxxx-xxxx-xxxx-xxxxxxx',
  '023xxx-xxxx-xxxx-xxx-xxxxxxx-xxxxxx-xxxxxxx',
  '024xxx-xxx-xxxx-xxx-xxxx-xxxx-xxx-xxxxxxxxx',
  '025xxx-xxxx-xxxx-xxxxxxxxx',
  '026xxx-xxx-xxxx-xxx-xxxxxxx',
  '027xxx-xxx-xxxx-xxx-xxxxxxx',
  '028xxx-xxx-xxxx-xxxxxxxxxx',
  '029xxx-xxx-xxxx-xxxxxx',
  '030xxx-xxx-xxxx-xxx-xxxxxxx',
  '031xxx-xxx-xxxx-xxx-xxxxxxxx',
  '032xxx-xxx-xxxxx-xxxxxxxxxxxx-xxxxxxx',
  '033xxx-xxx-xxxx-xxx-xxxx-xxxx',
  '034xxx-xxx-xxxx-xxx-xxxxxxxx-xxxxxxxx',
  '035xxx-xxx-xxxx-xxxxxxx',
  '036xxx-xxx-xxxx-xxxxxxxx-xxxxxxx',
  '037xxx-xxx-xxxx-xxxxxx',
  '038xxx-xxx-xxxx-xxxxxxxxxx',
  '039xxxx-xxx-xxxx-xxxxxx-xxxxxx-xxxxxxx',
  '040xxx-xxx-xxxx-xxxxxx-xxxxxxx',
  '041xxxx-xxx-xxxx-xxxxxx-xx',
  '042xxx-xxx-xxxx-xxxxxxxxxxx',
  '043xxx-xxx-xxxx-xxxxxxxxxxx-xxxxxxx',
  '044xxx-xxx-xxxx-xxxxxxx-xxx-xxxxxx',
  '045xxx-xxx-xxxx-xxxxxx-xxx-xxxxxx-xxx',
  '046xxx-xxx-xxxx-xxxxxx-xxx-xxxxxx-xxx',
  '047xxx-xxx-xxxx-xxxxxx-xxx-xxxxxx-xxxxxxx',
  '048xxx-xxx-xxxx-xxxxxx-xxx-xxxxxx-xxxxxxxx',
  '049xxx-xxx-xxxx-xxxxxx-xxx-xxxxxx-xxxxxxxxx',
  '050xxx-xxx-xxxxx-xxxxx-xxxxxxx',
  '051xxx-xxx-xxxx-xxxxx-xxxxxxxx-xxxxxx',
  '052xxx-xxx-xxxx-xxxxx-xx',
  '053xxx-xxx-xxxx-xxxx-xxxxxxx-xxx-xxxxxxx',
  '054xxx-xxx-xxxx-xxxx-xxxxxxx-xxx-xxxxxx',
  '055xxx-xxx-xxxx-xxx',
  '056xxx-xxx-xxxx-xxxxxxx-xxxxxxx',
  '057xxx-xxx-xxxx-xxxxxxx-xxx-xxxxxxx',
  '058xxx-xxx-xxxx-xxxxxxxxx-xxxxxx-xxxxxxx',
  '059xxx-xxx-xxxx-xx-xxxxxxxxxxx-xxxxx',
  '060xxx-xxx-xxxx-xxxxxxxxxxxxxx-xxxxxxx',
  '061xxx-xxx-xxxx-xxxxxxxx-xxxxxxx',
  '062xxx-xxx-xxxx-xxxxxxxxxxxxx-xxxxxxx',
  '063xxx-xxx-xxxx-xxxxxx-xxxxxxx',
  '064xxx-xxx-xxxxx-xxxxxx-xxxxxxx',
  '065xxx-xxx-xxxx-xxxxx-xxxxx',
  '066xxx-xxx-xxxx-xxxxx-xxxx',
  '067xxx-xxx-xxxx-xxxxxxxxxxxxxxxxx',
  '068xxx-xxx-xxxx-xxxxxxxx-xxxxx-xxx',
  '069xxx-xxx-xxxx-xxxxxxxx-xx-xxx-xxxxxxxx',
  '070xxx-xxx-xxxx-xxxxxxxx-xxxxxx-xxxxxxx',
  '071xxx-xxx-xxxx-xxxxxxxxxx',
  '072xxx-xxxx-xxxx-xxxxxxxxxxxxxxxx',
  '073xxx-xxx-xxxx-xxxxxx-xxxxxx-xxxxxxx',
  '074xxx-xxx-xxxx-xxxxxxx-xxxxxxxxxxxxx',
  '075xxx-xxx-xxxx-xxxxxxx-xxxxxxxxxxxxx-xxxxxxxxx',
  '076xxx-xxx-xxxx-xxxxxxxxxxxxx-xxxxxxx',
  '077xxx-xxx-xxxx-xxxxxxxxx-xxxxxxx-x2',
  '078xxx-xxx-xxxx-xxx',
  '079xxx-xxx-xxxx-xxxxxxxxxxxx',
  '080xxx-xxx-xxxx-xxxxxxxxx-xxxxxxxx',
  '081xxx-xxx-xxxx-xxxxxxxx-xxxxxxxxx',
  '082xxx-xxx-xxxx-xxxxxxxxx',
  '083xxx-xxx-xxxx-xxxxxxx',
  '084xxx-xxx-xxxx-xxxxxxx-xxxxxx',
  '085xxxx-xxx-xxxxxxx-xxxxx-xxxx',
  '086xxx-xxx-xxxxxxxx-xxxx-xxxxxxxxxx',
  '087xxx-xxx-xxxxxxx-xxxxxx-xxxxxxx',
  '088xxx-xxx-xxxxxxx-xxxxxxxx',
  '089xxx-xxx-xxxxxxxx-xxxxxxx',
  '090xxx-xxx-xxxxxxxx-xxxxxxx-xxxxxx',
  '091xxx-xxxx-xxxxxxxxxx',
  '092xxx-xxx-xxxxxx-xx',
  '093xxx-xxx-xxxxxxx',
  '094xxx-xxx-xxxxxxx-xxxxx',
  '095xxx-xxx-xxxxxxxxxx',
  '096xxx-xxx-xxxxxxxxxxxxxxx',
  '098xxx-xxx-xxxxxxxxxxxxxxxxx',
  '098xxx-xxx-xxxxxxxx-xxxxxxx-xxxxxxx',
  '099xxx-xxx-xxxxxxxx-xx',
  '100xxx-xxx-xxxx-xxxxxxxxxxx-xxxxxxx',
  '101xxx-xxxx-xxxxxxxxxxxxxxxx',
  '102xxx-xxx-xxxxxxxxxxxxxxxxxx',
  '103xxx-xxx-xxxxxxxx-xxxxxxxxxxxx-xxx',
  '104xxx-xxx-xxxx-xxxxxxx',
  '105xxx-xxx-xxx-xxxxxxx-xxxxxx-xxxxxxx',
  '106xxx-xxx-xxx-xxxx-xxxx-xxx-xxxxxxxxx',
  '107xxx-xxx-xxxxxxxxx',
  '108xxx-xxx-xxx-xxxxxxx',
  '109xxx-xxx-xxx-xxxxxxx',
  '110xxx-xxxx-xxxxx',
  '111xxx-xxx-xxx-xxxx-xxxx',
  '112xxx-xxx-xxx-xxxxxxx',
  '113xxx-xxx-xxx-xxxxxxx',
  '114xxx-xxx-xxxxxxxxxxxx-xxxxxxx',
  '115xxx-xxx-xxx-xxxx-xxxx',
  '116xxx-xxx-xxx-xxxxxxxx-xxxxxxxx',
  '117xxx-xxx-xxxxxxx',
  '118xxx-xxx-xxxxxxxx-xxxxxxx',
  '119xxx-xxx-xxxxxx',
  '120xxxx-xxx-xxxxxxxxxx',
  '121xxx-xxx-xxxxxx-xxxxxx-xxxxxxx',
  '122xxx-xxx-xxxxxx-xxxxxxx',
  '123xxx-xxx-xxxxxx-xx',
  '124xxxx-xxx-xxx-xxxx-xxxxxxx',
  '125xxx-xxx-xxxxxxxxxxx',
  '126xxx-xxx-xxxxxxxxxxx-xxxxxxx',
  '127xxx-xxx-xxxxxx-xxx-xxxxxx',
  '128xxx-xxx-xxxxxxxxxx',
  '129xxx-xxx-xxxxxxxxxx-xxx',
  '130xxx-xxx-xxxxx-xxxxxxx',
  '131xxx-xxx-xxxxxx-xxxxxxxx-xxx',
  '132xxx-xxx-xxxxx-xxxxxxxx-xxxxxx',
  '133xxx-xxx-xxxxx-xx',
  '134xxx-xxx-xxxx-xxxxxxx-xxx-xxxxxxx',
  '135xxx-xxx-xxxx-xxxxxxx-xxx-xxxxxx',
  '136xxx-xxx-xxxxxxxxxxxxxxx',
  '137xxx-xxx-xxx',
  '138xxx-xxx-xxxxxxx-xxxxxxx',
  '139xxx-xxx-xxxxxxx-xxx-xxxxxxx',
  '140xxx-xxx-xxxxxxxxx-xxxxxx-xxxxxxx',
  '141xxx-xxx-xxxxxxxxxx-xxxx-xxxxxxx',
  '142xxx-xxx-xx-xxxxxxxxxx-xxxxx',
  '143xxx-xxx-xxxxxxxxxxxxxx-xxxxxxx',
  '144xxx-xxx-xxxxxxxxxxxxxxxxx',
  '145xxx-xxx-xxxxxxxx-xxxxxxx',
  '146xxx-xxx-xxxxxxxxxxxxx-xxxxxxx',
  '147xxx-xxx-xxxxxxx-xxxxxxx',
  '148xxx-xxx-xxxxxx-xxxxxxx',
  '149xxx-xxx-xxxxx-xxxxx',
  '150xxx-xxx-xxxxx-xxxx',
  '151xxx-xxx-xxxxxxxxxxxxxxxxx',
  '152xxx-xxx-xxxxxxxxxxxxxxxxxxx',
  '153xxx-xxx-xxxxxxxxx-xxxxx-xxx',
  '154xxx-xxx-xxxxxxxx-xxxxx-xxxx-xxxx',
  '155xxx-xxx-xxxxxxxx-xxxxxxxxxxxx-xxxx',
  '156xxx-xxx-xxxxxxxx-xx-xxx-xxxxxxxx',
  '157xxx-xxx-xxxxxxxx-xxxxxx-xxxxxxx',
  '158xxx-xxx-xxxxxxxxxx',
  '159xxx-xxx-xxxx-xxxxxx',
  '160xxx-xxx-xxxxxxxxxx',
  '161xxx-xxx-xxxxxxxxxxxxx',
  '162xxx-xxx-xxxxxxxxxxxxxxxx',
  '163xxx-xxx-xxxxx-xxxxxx-xxxxxxx',
  '164xxx-xxx-xxxxxxx-xxxxxxxxxxxxx',
  '165xxx-xxx-xxxxxxxxxxxxxx-xxxxxxx',
  '166xxx-xxx-xxxxxxxxx-xxxxxxx-x2',
  '167xxx-xxx-xxx',
  '168xxx-xxx-xxxxxxxxxxxx',
  '169xxx-xxx-xxxxxxxxx-xxxxxxxx',
  '170xxx-xxx-xxxxxxxxx-xxx-xxxxxxx',
  '171xxx-xxx-xxxxxxxxx',
  '172xxx-xxxx-xxxxxxxxx',
  '173xxx-xxx',
  '174xxx-xxxxxx-xxxxxxx',
  '175xxx-xxxxxx-xxxxxx-xxxx',
  '176xxx-xxxxxx-xxxxx',
  '177xxx-xxxxx-xxxxxxxx',
  '178xxx-xxxxxx',
  '179xxx-xxxxxxx-xxxx',
  '180xxx-xxxxxxx-xxxx-xxxx',
  '181xxx-xxxxxxx-xxxx-xxxxxxx',
  '182xxx-xxxxxxx-xxxx-xxxxxxxxxxxxxxx',
  '183xxx-xxxxxxxx-xxxx-xxxxx',
  '184xxx-xxxxxxx-xxxx-xxxxxxx',
  '185xxx-xxxxxxx-xxxx-xxxxxxxx-xxxxxxx',
  '186xxx-xxxxxxx-xxxx-xxx',
  '187xxx-xxxxxxx-xxxx-xxxxxxx-xxx-xxxxxxx',
  '188xxx-xxxxxxx-xxxx-xxxxxxxxxxxxxx-xxxxxxx',
  '189xxx-xxxxxxxx-xxxx-xxxxxxxxxxxxxxxxx',
  '190xxx-xxxxxxx-xxxx-xxxxx-xxxxxx-xxxxxxx',
  '191xxx-xxxxxxx-xxxxxxxxxx',
  '192xxx-xxxxxxx-xxxxxxx',
  '193xxx-xxxxxxx-xxxxxxxxxxxxxxx',
  '194xxx-xxxxxxx-xxxxx',
  '195xxx-xxxxxxx-xxxxxxx',
  '196xxx-xxxxxxx-xxxxxxxx-xxxxxxx',
  '197xxx-xxxxxxx-xxx',
  '198xxx-xxxxxxx-xxxxxxx-xxx-xxxxxxx',
  '199xxx-xxxxxxx-xxxxxxxxxxxxxx-xxxxxxx',
  '200xxx-xxxxxxxx-xxxxxxxxxxxxxxxxx',
  '201xxx-xxxxxxx-xxxxx-xxxxxx-xxxxxxx',
  '202xxx-xxx',
  '203xxx-xxx-xxxxxx',
  '204xxxxxxxxx-xxx',
  '205xxxxxxxxxxx-xxx',
  '206xxxxxxxxx-xxxxxxx',
  '207xxxx-xxxxxx-xxxxxxxxxxx-xxxxx1',
  '208xxxx-xxxxxxxx-xxxxxxxx-xxxxx1',
  '209xxxx-xxxxxxx-xxxxxxx-xxxxx1',
  '209xxxx-xxxxxxx-xxxxxxx-xxxxx2',
  '210xxxx-xxxx-xxxxxxx-xxxxx1',
  '211xxxx-xxxxx-xxxxxxxx1-xxxxx1',
  '212xxxx-xxxx-xxxxxxxx2-xxxxx1',
  '213xxxx-xxxx-xxxxxxx1-xxxxx1',
  '214xxxx-xxxx-xxxxxxx2-xxxxx1',
  '215xxxxxxxxx-xxxxxx',
  '216xxxxxxxxx-xxxxxxx',
  '217xxxxxxxxx-xxxx',
];

{
  'yyyyyyyyy': {
    local Func3(z, n) = utils.Func3('yyyyyyyyy', z, n),

    global: {
      [n]: {
        members: [
          Func3(z, '%s-gggg-%s-%s' % [n, z, suffix]),
          for z in ['ooooooo-a', 'ooooooo-b', 'ooooooo-c',
                       'ooooooo-a', 'ooooooo-b', 'ooooooo-c']
          for suffix in ['a', 'b', 'c']
        ],
      }
      for n in long_list
    },
  },
}

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 14.6 ± 0.6 | 13.5 | 17.9 | 1.00 |
| `Go` | 7663.9 ± 114.5 | 7518.9 | 7912.3 | 524.74 ± 24.02 |
| `Scala` | 414.1 ± 5.4 | 406.8 | 429.4 | 28.35 ± 1.28 |
| `C++` | 26376.7 ± 354.2 | 25755.3 | 26908.5 | 1806.00 ± 81.81 |

### Realistic 2

<details>
<summary>Source</summary>

```jsonnet
local rfc3339(timestamp) = '1970-01-01T00:00:00Z';

local name1(a, b, c) =
  'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA%sBBBBBBB%sCCCCCCCCCCC%s'
  % [a, b, c];

local name2(x) =
  'XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX%s' % x;

local T1 = 'PPPPPPPPPPPPPPPPPPPPPPP';
local T2 = 'QQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQQ';

local LOCATIONS = [
  'europe-west1-b',
  'europe-west1-c',
  'europe-west1-d',
  'europe-west2-a',
  'europe-west2-b',
  'europe-west2-c',
  'europe-west3-a',
  'europe-west3-b',
  'europe-west3-c',
  'europe-west4-a',
  'europe-west4-b',
  'europe-west4-c',
  'us-central1-a',
  'us-central1-b',
  'us-central1-c',
  'us-central1-f',
  'us-east1-b',
  'us-east1-c',
  'us-east1-d',
  'us-east4-a',
  'us-east4-b',
  'us-east4-c',
  'us-west1-a',
  'us-west1-b',
  'us-west1-c',
];


// The return value is a function to allow it to be parameterized.
function(
  timestamp=0,
  x='xxxxxxxxxxxxxxxxxxx',
  prefix='prefix',
  num1=50,
  count=25,
  offset=0,
)
  local rfc_timestamp = rfc3339(timestamp);

  local func1(i) =
    local location = LOCATIONS[i % std.length(LOCATIONS)];
    [
      local name = '%s-%000d-%000d' % [prefix, i, j];
      {
        field_zz1: rfc_timestamp,
        fie_z2: {
          field_z3: name1(x, location, name),
          field_zzzzzzzzzz4: name2(x),
          field_zzz5: [name],
          field_z6: T1,
          field_z7: location,
          fi_8: '???',
          fiel_z9: '99',
        },
      }
      for j in std.range(0, num1 - 1)
    ];

  local func2(i) =
    local location = LOCATIONS[i % std.length(LOCATIONS)];
    local all = [
      name1(x, location, '%s-%000d-%000d' % [prefix, i, j])
      for j in std.range(0, num1 - 1)
    ];
    [
      {
        field_yy1: rfc_timestamp,
        field_yyyyy2: 'EEEE',
        field_yyyyy3: {
          field_y4: T2,
          field_yyyyyyy5: p,
          field_yyyyyyy6: q,
        },
      }
      for p in all
      for q in all
      if p != q
    ];


  {
    field_x1: '-----',
    field_xxxxxxxxxxxxxxxx2: std.join([], [
      func1(offset * count + i)
      for i in std.range(0, count - 1)
    ]),
    field_xxxxxxxxxxxxxxxxxxxxxx3: std.join([], [
      func2(offset * count + i)
      for i in std.range(0, count - 1)
    ]),
  }


```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 295.6 ± 8.9 | 285.1 | 319.9 | 1.00 |
| `Go` | 7540.7 ± 107.5 | 7357.3 | 7792.5 | 25.51 ± 0.85 |
| `Scala` | 781.0 ± 27.4 | 745.3 | 854.7 | 2.64 ± 0.12 |
| `C++` | 30144.1 ± 574.5 | 28895.0 | 30967.3 | 101.99 ± 3.62 |

## Benchmarks from C++ jsonnet (/benchmarks)

### Tail call

<details>
<summary>Source</summary>

```jsonnet
/*
Copyright 2015 Google Inc. All rights reserved.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

local sum(x) =
  if x == 0 then
    0
  else
    x + sum(x - 1);
sum(300)

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 2.2 ± 0.1 | 1.9 | 3.6 | 1.00 |
| `Go` | 6.7 ± 0.6 | 6.0 | 17.0 | 3.10 ± 0.32 |
| `Scala` | 306.5 ± 2.1 | 302.8 | 311.8 | 141.78 ± 7.81 |
| `C++` | 34.7 ± 2.2 | 32.6 | 52.1 | 16.04 ± 1.35 |

### Inheritance recursion

<details>
<summary>Source</summary>

```jsonnet
/*
Copyright 2015 Google Inc. All rights reserved.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

local Fib = {
  n: 1,
  local outer = self,
  r: if self.n <= 1 then 1 else (Fib { n: outer.n - 1 }).r + (Fib { n: outer.n - 2 }).r,
};

(Fib { n: 25 }).r

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 452.8 ± 20.3 | 425.2 | 494.6 | 1.00 |
| `Go` | 1076.4 ± 16.2 | 1049.9 | 1111.1 | 2.38 ± 0.11 |
| `Scala` | 475.5 ± 8.7 | 462.2 | 494.4 | 1.05 ± 0.05 |
| `C++` | 3029.7 ± 121.1 | 2787.2 | 3214.5 | 6.69 ± 0.40 |

### Simple recursive call

<details>
<summary>Source</summary>

```jsonnet
/*
Copyright 2015 Google Inc. All rights reserved.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

local fibonacci(n) =
  if n <= 1 then
    1
  else
    fibonacci(n - 1) + fibonacci(n - 2);

fibonacci(25)

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 109.2 ± 2.4 | 104.8 | 114.3 | 1.00 |
| `Go` | 429.0 ± 8.2 | 419.4 | 454.3 | 3.93 ± 0.12 |
| `Scala` | 338.7 ± 3.2 | 334.6 | 345.6 | 3.10 ± 0.07 |
| `C++` | 210.0 ± 8.0 | 195.5 | 227.5 | 1.92 ± 0.08 |

### Foldl string concat

<details>
<summary>Source</summary>

```jsonnet
std.foldl(function(e, res) e + res, std.makeArray(20000, function(i) 'aaaaa'), '')

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 12.5 ± 0.6 | 11.4 | 17.3 | 1.00 |
| `Go` | 64.8 ± 3.1 | 60.8 | 78.8 | 5.16 ± 0.36 |
| `Scala` | 630.6 ± 15.6 | 606.3 | 659.8 | 50.25 ± 2.83 |
| `C++` | 1073.3 ± 18.2 | 1041.0 | 1107.6 | 85.53 ± 4.56 |

### Array sorts

> Note: No results for Scala, std.reverse is not implemented

<details>
<summary>Source</summary>

```jsonnet
// A benchmark for builtin sort

local reverse = std.reverse;
local sort = std.sort;

true
&& std.assertEqual(std.range(1, 500), sort(std.range(1, 500)))
&& std.assertEqual(std.range(1, 1000), sort(std.range(1, 1000)))
&& std.assertEqual(reverse(std.range(1, 1000)), sort(std.range(1, 1000), keyF=function(x) -x))
&& std.assertEqual(std.range(1, 1000), sort(reverse(std.range(1, 1000))))
&& std.assertEqual(std.makeArray(2000, function(i) std.floor((i + 2) / 2)), sort(std.range(1, 1000) + reverse(std.range(1, 1000))))

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 4.2 ± 0.1 | 3.9 | 4.9 | 1.00 |
| `Go` | 14.8 ± 0.5 | 13.7 | 17.8 | 3.51 ± 0.17 |
| `C++` | 3964.0 ± 187.1 | 3700.2 | 4225.1 | 939.25 ± 55.45 |

### Lazy array

<details>
<summary>Source</summary>

```jsonnet
local f2(f) = function(x) f(f(x));
local id(x) = x;

local slowId = std.makeArray(20, function(i) if i == 0 then id else f2(slowId[i - 1]));

slowId[15](42)

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 58.8 ± 2.0 | 55.6 | 64.2 | 1.00 |
| `Go` | 661.6 ± 26.6 | 629.6 | 730.1 | 11.25 ± 0.59 |
| `Scala` | 348.8 ± 6.6 | 339.7 | 365.3 | 5.93 ± 0.23 |
| `C++` | 206.4 ± 3.3 | 202.5 | 215.2 | 3.51 ± 0.13 |

### Inheritance function recursion

<details>
<summary>Source</summary>

```jsonnet
local fibnext = {
  a: super.a + super.b,
  b: super.a,
};
local fib(n) =
  if n == 0 then
    { a: 1, b: 1 }
  else
    fib(n - 1) + fibnext;

fib(25)

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 1.9 ± 0.1 | 1.6 | 3.3 | 1.00 |
| `Go` | 3.0 ± 0.2 | 2.7 | 5.1 | 1.58 ± 0.15 |
| `Scala` | 310.0 ± 9.5 | 299.9 | 341.1 | 161.51 ± 9.88 |
| `C++` | 1505.6 ± 74.0 | 1363.5 | 1640.8 | 784.26 ± 56.71 |

### String strips

<details>
<summary>Source</summary>

```jsonnet
// This string must be longer than max stack frames
local veryLongString = std.join('', std.repeat(['e'], 510));

std.assertEqual(std.stripChars(veryLongString + 'ok' + veryLongString, 'e'), 'ok') &&
std.assertEqual(std.lstripChars(veryLongString + 'ok', 'e'), 'ok') &&
std.assertEqual(std.rstripChars('ok' + veryLongString, 'e'), 'ok') &&

true

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 9.4 ± 0.3 | 8.7 | 10.4 | 1.00 |
| `Go` | 15.0 ± 0.5 | 13.9 | 17.6 | 1.60 ± 0.08 |
| `Scala` | 340.6 ± 5.4 | 333.3 | 359.9 | 36.39 ± 1.36 |
| `C++` | 37161.7 ± 525.9 | 36471.3 | 38275.4 | 3970.49 ± 145.97 |

### Big object

<details>
<summary>Source</summary>

```jsonnet
// Generator source
local n = 2000;

local objLocal(name, body) = 'local ' + name + ' = ' + body + ',';
local objField(name, body) = name + ': ' + body + ',';

local allLocals =
  std.makeArray(n, function(i) objLocal('l' + i, '1'));

local allFields =
  std.makeArray(n, function(i) objField('f' + i, '2'));

local indent = '  ';
local indentAndSeparate(s) = indent + s + '\n';

local objContents = std.map(indentAndSeparate, allLocals + allFields);

local objectBody = std.join('', objContents);
'{\n' + objectBody + '}\n'

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 2.5 ± 0.1 | 2.3 | 3.3 | 1.00 |
| `Go` | 5.2 ± 0.3 | 4.6 | 7.0 | 2.07 ± 0.15 |
| `Scala` | 302.6 ± 6.4 | 290.3 | 312.9 | 120.34 ± 6.71 |
| `C++` | 29.4 ± 1.4 | 27.7 | 38.5 | 11.71 ± 0.82 |

## Benchmarks from Go jsonnet (builtins)

### std.base64

<details>
<summary>Source</summary>

```jsonnet
{
    foo: [
        std.base64("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Sed turpis tincidunt id aliquet risus. Eget mauris pharetra et ultrices neque ornare aenean euismod. Diam quis enim lobortis scelerisque fermentum. Varius duis at consectetur lorem donec massa sapien. Diam sit amet nisl suscipit adipiscing bibendum est ultricies integer. Lectus urna duis convallis convallis tellus. Nibh ipsum consequat nisl vel pretium lectus quam id leo. Feugiat in ante metus dictum at tempor commodo. Velit dignissim sodales ut eu sem integer. Dictum sit amet justo donec. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Sed turpis tincidunt id aliquet risus. Eget mauris pharetra et ultrices neque ornare aenean euismod. Diam quis enim lobortis scelerisque fermentum. Varius duis at consectetur lorem donec massa sapien. Diam sit amet nisl suscipit adipiscing bibendum est ultricies integer. Lectus urna duis convallis convallis tellus. Nibh ipsum consequat nisl vel pretium lectus quam id leo. Feugiat in ante metus dictum at tempor commodo. Velit dignissim sodales ut eu sem integer. Dictum sit amet justo donec. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Sed turpis tincidunt id aliquet risus. Eget mauris pharetra et ultrices neque ornare aenean euismod. Diam quis enim lobortis scelerisque fermentum. Varius duis at consectetur lorem donec massa sapien. Diam sit amet nisl suscipit adipiscing bibendum est ultricies integer. Lectus urna duis convallis convallis tellus. Nibh ipsum consequat nisl vel pretium lectus quam id leo. Feugiat in ante metus dictum at tempor commodo. Velit dignissim sodales ut eu sem integer. Dictum sit amet justo donec. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus.Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Sed turpis tincidunt id aliquet risus. Eget mauris pharetra et ultrices neque ornare aenean euismod. Diam quis enim lobortis scelerisque fermentum. Varius duis at consectetur lorem donec massa sapien. Diam sit amet nisl suscipit adipiscing bibendum est ultricies integer. Lectus urna duis convallis convallis tellus. Nibh ipsum consequat nisl vel pretium lectus quam id leo. Feugiat in ante metus dictum at tempor commodo. Velit dignissim sodales ut eu sem integer. Dictum sit amet justo donec. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Sed turpis tincidunt id aliquet risus. Eget mauris pharetra et ultrices neque ornare aenean euismod. Diam quis enim lobortis scelerisque fermentum. Varius duis at consectetur lorem donec massa sapien. Diam sit amet nisl suscipit adipiscing bibendum est ultricies integer. Lectus urna duis convallis convallis tellus. Nibh ipsum consequat nisl vel pretium lectus quam id leo. Feugiat in ante metus dictum at tempor commodo. Velit dignissim sodales ut eu sem integer. Dictum sit amet justo donec. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus.") for i in std.range(0,100)
    ],
}
```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 3.4 ± 0.1 | 3.1 | 4.9 | 1.00 |
| `Go` | 21.6 ± 0.8 | 19.7 | 27.1 | 6.32 ± 0.34 |
| `Scala` | 355.0 ± 6.0 | 340.7 | 367.4 | 103.77 ± 4.62 |
| `C++` | 16359.4 ± 663.5 | 15526.0 | 17805.8 | 4782.25 ± 276.36 |

### std.base64Decode

<details>
<summary>Source</summary>

```jsonnet
{
    foo: [
        std.base64Decode("TG9yZW0gaXBzdW0gZG9sb3Igc2l0IGFtZXQsIGNvbnNlY3RldHVyIGFkaXBpc2NpbmcgZWxpdCwgc2VkIGRvIGVpdXNtb2QgdGVtcG9yIGluY2lkaWR1bnQgdXQgbGFib3JlIGV0IGRvbG9yZSBtYWduYSBhbGlxdWEuIFNlZCB0dXJwaXMgdGluY2lkdW50IGlkIGFsaXF1ZXQgcmlzdXMuIEVnZXQgbWF1cmlzIHBoYXJldHJhIGV0IHVsdHJpY2VzIG5lcXVlIG9ybmFyZSBhZW5lYW4gZXVpc21vZC4gRGlhbSBxdWlzIGVuaW0gbG9ib3J0aXMgc2NlbGVyaXNxdWUgZmVybWVudHVtLiBWYXJpdXMgZHVpcyBhdCBjb25zZWN0ZXR1ciBsb3JlbSBkb25lYyBtYXNzYSBzYXBpZW4uIERpYW0gc2l0IGFtZXQgbmlzbCBzdXNjaXBpdCBhZGlwaXNjaW5nIGJpYmVuZHVtIGVzdCB1bHRyaWNpZXMgaW50ZWdlci4gTGVjdHVzIHVybmEgZHVpcyBjb252YWxsaXMgY29udmFsbGlzIHRlbGx1cy4gTmliaCBpcHN1bSBjb25zZXF1YXQgbmlzbCB2ZWwgcHJldGl1bSBsZWN0dXMgcXVhbSBpZCBsZW8uIEZldWdpYXQgaW4gYW50ZSBtZXR1cyBkaWN0dW0gYXQgdGVtcG9yIGNvbW1vZG8uIFZlbGl0IGRpZ25pc3NpbSBzb2RhbGVzIHV0IGV1IHNlbSBpbnRlZ2VyLiBEaWN0dW0gc2l0IGFtZXQganVzdG8gZG9uZWMuIFNjZWxlcmlzcXVlIG1hdXJpcyBwZWxsZW50ZXNxdWUgcHVsdmluYXIgcGVsbGVudGVzcXVlIGhhYml0YW50IG1vcmJpIHRyaXN0aXF1ZSBzZW5lY3R1cy4gTG9yZW0gaXBzdW0gZG9sb3Igc2l0IGFtZXQsIGNvbnNlY3RldHVyIGFkaXBpc2NpbmcgZWxpdCwgc2VkIGRvIGVpdXNtb2QgdGVtcG9yIGluY2lkaWR1bnQgdXQgbGFib3JlIGV0IGRvbG9yZSBtYWduYSBhbGlxdWEuIFNlZCB0dXJwaXMgdGluY2lkdW50IGlkIGFsaXF1ZXQgcmlzdXMuIEVnZXQgbWF1cmlzIHBoYXJldHJhIGV0IHVsdHJpY2VzIG5lcXVlIG9ybmFyZSBhZW5lYW4gZXVpc21vZC4gRGlhbSBxdWlzIGVuaW0gbG9ib3J0aXMgc2NlbGVyaXNxdWUgZmVybWVudHVtLiBWYXJpdXMgZHVpcyBhdCBjb25zZWN0ZXR1ciBsb3JlbSBkb25lYyBtYXNzYSBzYXBpZW4uIERpYW0gc2l0IGFtZXQgbmlzbCBzdXNjaXBpdCBhZGlwaXNjaW5nIGJpYmVuZHVtIGVzdCB1bHRyaWNpZXMgaW50ZWdlci4gTGVjdHVzIHVybmEgZHVpcyBjb252YWxsaXMgY29udmFsbGlzIHRlbGx1cy4gTmliaCBpcHN1bSBjb25zZXF1YXQgbmlzbCB2ZWwgcHJldGl1bSBsZWN0dXMgcXVhbSBpZCBsZW8uIEZldWdpYXQgaW4gYW50ZSBtZXR1cyBkaWN0dW0gYXQgdGVtcG9yIGNvbW1vZG8uIFZlbGl0IGRpZ25pc3NpbSBzb2RhbGVzIHV0IGV1IHNlbSBpbnRlZ2VyLiBEaWN0dW0gc2l0IGFtZXQganVzdG8gZG9uZWMuIFNjZWxlcmlzcXVlIG1hdXJpcyBwZWxsZW50ZXNxdWUgcHVsdmluYXIgcGVsbGVudGVzcXVlIGhhYml0YW50IG1vcmJpIHRyaXN0aXF1ZSBzZW5lY3R1cy4gTG9yZW0gaXBzdW0gZG9sb3Igc2l0IGFtZXQsIGNvbnNlY3RldHVyIGFkaXBpc2NpbmcgZWxpdCwgc2VkIGRvIGVpdXNtb2QgdGVtcG9yIGluY2lkaWR1bnQgdXQgbGFib3JlIGV0IGRvbG9yZSBtYWduYSBhbGlxdWEuIFNlZCB0dXJwaXMgdGluY2lkdW50IGlkIGFsaXF1ZXQgcmlzdXMuIEVnZXQgbWF1cmlzIHBoYXJldHJhIGV0IHVsdHJpY2VzIG5lcXVlIG9ybmFyZSBhZW5lYW4gZXVpc21vZC4gRGlhbSBxdWlzIGVuaW0gbG9ib3J0aXMgc2NlbGVyaXNxdWUgZmVybWVudHVtLiBWYXJpdXMgZHVpcyBhdCBjb25zZWN0ZXR1ciBsb3JlbSBkb25lYyBtYXNzYSBzYXBpZW4uIERpYW0gc2l0IGFtZXQgbmlzbCBzdXNjaXBpdCBhZGlwaXNjaW5nIGJpYmVuZHVtIGVzdCB1bHRyaWNpZXMgaW50ZWdlci4gTGVjdHVzIHVybmEgZHVpcyBjb252YWxsaXMgY29udmFsbGlzIHRlbGx1cy4gTmliaCBpcHN1bSBjb25zZXF1YXQgbmlzbCB2ZWwgcHJldGl1bSBsZWN0dXMgcXVhbSBpZCBsZW8uIEZldWdpYXQgaW4gYW50ZSBtZXR1cyBkaWN0dW0gYXQgdGVtcG9yIGNvbW1vZG8uIFZlbGl0IGRpZ25pc3NpbSBzb2RhbGVzIHV0IGV1IHNlbSBpbnRlZ2VyLiBEaWN0dW0gc2l0IGFtZXQganVzdG8gZG9uZWMuIFNjZWxlcmlzcXVlIG1hdXJpcyBwZWxsZW50ZXNxdWUgcHVsdmluYXIgcGVsbGVudGVzcXVlIGhhYml0YW50IG1vcmJpIHRyaXN0aXF1ZSBzZW5lY3R1cy5Mb3JlbSBpcHN1bSBkb2xvciBzaXQgYW1ldCwgY29uc2VjdGV0dXIgYWRpcGlzY2luZyBlbGl0LCBzZWQgZG8gZWl1c21vZCB0ZW1wb3IgaW5jaWRpZHVudCB1dCBsYWJvcmUgZXQgZG9sb3JlIG1hZ25hIGFsaXF1YS4gU2VkIHR1cnBpcyB0aW5jaWR1bnQgaWQgYWxpcXVldCByaXN1cy4gRWdldCBtYXVyaXMgcGhhcmV0cmEgZXQgdWx0cmljZXMgbmVxdWUgb3JuYXJlIGFlbmVhbiBldWlzbW9kLiBEaWFtIHF1aXMgZW5pbSBsb2JvcnRpcyBzY2VsZXJpc3F1ZSBmZXJtZW50dW0uIFZhcml1cyBkdWlzIGF0IGNvbnNlY3RldHVyIGxvcmVtIGRvbmVjIG1hc3NhIHNhcGllbi4gRGlhbSBzaXQgYW1ldCBuaXNsIHN1c2NpcGl0IGFkaXBpc2NpbmcgYmliZW5kdW0gZXN0IHVsdHJpY2llcyBpbnRlZ2VyLiBMZWN0dXMgdXJuYSBkdWlzIGNvbnZhbGxpcyBjb252YWxsaXMgdGVsbHVzLiBOaWJoIGlwc3VtIGNvbnNlcXVhdCBuaXNsIHZlbCBwcmV0aXVtIGxlY3R1cyBxdWFtIGlkIGxlby4gRmV1Z2lhdCBpbiBhbnRlIG1ldHVzIGRpY3R1bSBhdCB0ZW1wb3IgY29tbW9kby4gVmVsaXQgZGlnbmlzc2ltIHNvZGFsZXMgdXQgZXUgc2VtIGludGVnZXIuIERpY3R1bSBzaXQgYW1ldCBqdXN0byBkb25lYy4gU2NlbGVyaXNxdWUgbWF1cmlzIHBlbGxlbnRlc3F1ZSBwdWx2aW5hciBwZWxsZW50ZXNxdWUgaGFiaXRhbnQgbW9yYmkgdHJpc3RpcXVlIHNlbmVjdHVzLiBMb3JlbSBpcHN1bSBkb2xvciBzaXQgYW1ldCwgY29uc2VjdGV0dXIgYWRpcGlzY2luZyBlbGl0LCBzZWQgZG8gZWl1c21vZCB0ZW1wb3IgaW5jaWRpZHVudCB1dCBsYWJvcmUgZXQgZG9sb3JlIG1hZ25hIGFsaXF1YS4gU2VkIHR1cnBpcyB0aW5jaWR1bnQgaWQgYWxpcXVldCByaXN1cy4gRWdldCBtYXVyaXMgcGhhcmV0cmEgZXQgdWx0cmljZXMgbmVxdWUgb3JuYXJlIGFlbmVhbiBldWlzbW9kLiBEaWFtIHF1aXMgZW5pbSBsb2JvcnRpcyBzY2VsZXJpc3F1ZSBmZXJtZW50dW0uIFZhcml1cyBkdWlzIGF0IGNvbnNlY3RldHVyIGxvcmVtIGRvbmVjIG1hc3NhIHNhcGllbi4gRGlhbSBzaXQgYW1ldCBuaXNsIHN1c2NpcGl0IGFkaXBpc2NpbmcgYmliZW5kdW0gZXN0IHVsdHJpY2llcyBpbnRlZ2VyLiBMZWN0dXMgdXJuYSBkdWlzIGNvbnZhbGxpcyBjb252YWxsaXMgdGVsbHVzLiBOaWJoIGlwc3VtIGNvbnNlcXVhdCBuaXNsIHZlbCBwcmV0aXVtIGxlY3R1cyBxdWFtIGlkIGxlby4gRmV1Z2lhdCBpbiBhbnRlIG1ldHVzIGRpY3R1bSBhdCB0ZW1wb3IgY29tbW9kby4gVmVsaXQgZGlnbmlzc2ltIHNvZGFsZXMgdXQgZXUgc2VtIGludGVnZXIuIERpY3R1bSBzaXQgYW1ldCBqdXN0byBkb25lYy4gU2NlbGVyaXNxdWUgbWF1cmlzIHBlbGxlbnRlc3F1ZSBwdWx2aW5hciBwZWxsZW50ZXNxdWUgaGFiaXRhbnQgbW9yYmkgdHJpc3RpcXVlIHNlbmVjdHVzLiBTY2VsZXJpc3F1ZSBtYXVyaXMgcGVsbGVudGVzcXVlIHB1bHZpbmFyIHBlbGxlbnRlc3F1ZSBoYWJpdGFudCBtb3JiaSB0cmlzdGlxdWUgc2VuZWN0dXMuIFNjZWxlcmlzcXVlIG1hdXJpcyBwZWxsZW50ZXNxdWUgcHVsdmluYXIgcGVsbGVudGVzcXVlIGhhYml0YW50IG1vcmJpIHRyaXN0aXF1ZSBzZW5lY3R1cy4gU2NlbGVyaXNxdWUgbWF1cmlzIHBlbGxlbnRlc3F1ZSBwdWx2aW5hciBwZWxsZW50ZXNxdWUgaGFiaXRhbnQgbW9yYmkgdHJpc3RpcXVlIHNlbmVjdHVzLiBTY2VsZXJpc3F1ZSBtYXVyaXMgcGVsbGVudGVzcXVlIHB1bHZpbmFyIHBlbGxlbnRlc3F1ZSBoYWJpdGFudCBtb3JiaSB0cmlzdGlxdWUgc2VuZWN0dXMuIFNjZWxlcmlzcXVlIG1hdXJpcyBwZWxsZW50ZXNxdWUgcHVsdmluYXIgcGVsbGVudGVzcXVlIGhhYml0YW50IG1vcmJpIHRyaXN0aXF1ZSBzZW5lY3R1cy4=") for i in std.range(0,100)
    ],
}
```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 3.4 ± 0.2 | 3.1 | 5.3 | 1.00 |
| `Go` | 21.0 ± 1.5 | 19.6 | 32.2 | 6.24 ± 0.52 |
| `Scala` | 358.3 ± 7.1 | 349.0 | 379.9 | 106.23 ± 5.27 |
| `C++` | 10921.3 ± 240.4 | 10653.9 | 11346.7 | 3237.97 ± 163.58 |

### std.base64DecodeBytes

<details>
<summary>Source</summary>

```jsonnet
{
    foo: [
        std.base64DecodeBytes("TG9yZW0gaXBzdW0gZG9sb3Igc2l0IGFtZXQsIGNvbnNlY3RldHVyIGFkaXBpc2NpbmcgZWxpdCwgc2VkIGRvIGVpdXNtb2QgdGVtcG9yIGluY2lkaWR1bnQgdXQgbGFib3JlIGV0IGRvbG9yZSBtYWduYSBhbGlxdWEuIFNlZCB0dXJwaXMgdGluY2lkdW50IGlkIGFsaXF1ZXQgcmlzdXMuIEVnZXQgbWF1cmlzIHBoYXJldHJhIGV0IHVsdHJpY2VzIG5lcXVlIG9ybmFyZSBhZW5lYW4gZXVpc21vZC4gRGlhbSBxdWlzIGVuaW0gbG9ib3J0aXMgc2NlbGVyaXNxdWUgZmVybWVudHVtLiBWYXJpdXMgZHVpcyBhdCBjb25zZWN0ZXR1ciBsb3JlbSBkb25lYyBtYXNzYSBzYXBpZW4uIERpYW0gc2l0IGFtZXQgbmlzbCBzdXNjaXBpdCBhZGlwaXNjaW5nIGJpYmVuZHVtIGVzdCB1bHRyaWNpZXMgaW50ZWdlci4gTGVjdHVzIHVybmEgZHVpcyBjb252YWxsaXMgY29udmFsbGlzIHRlbGx1cy4gTmliaCBpcHN1bSBjb25zZXF1YXQgbmlzbCB2ZWwgcHJldGl1bSBsZWN0dXMgcXVhbSBpZCBsZW8uIEZldWdpYXQgaW4gYW50ZSBtZXR1cyBkaWN0dW0gYXQgdGVtcG9yIGNvbW1vZG8uIFZlbGl0IGRpZ25pc3NpbSBzb2RhbGVzIHV0IGV1IHNlbSBpbnRlZ2VyLiBEaWN0dW0gc2l0IGFtZXQganVzdG8gZG9uZWMuIFNjZWxlcmlzcXVlIG1hdXJpcyBwZWxsZW50ZXNxdWUgcHVsdmluYXIgcGVsbGVudGVzcXVlIGhhYml0YW50IG1vcmJpIHRyaXN0aXF1ZSBzZW5lY3R1cy4gTG9yZW0gaXBzdW0gZG9sb3Igc2l0IGFtZXQsIGNvbnNlY3RldHVyIGFkaXBpc2NpbmcgZWxpdCwgc2VkIGRvIGVpdXNtb2QgdGVtcG9yIGluY2lkaWR1bnQgdXQgbGFib3JlIGV0IGRvbG9yZSBtYWduYSBhbGlxdWEuIFNlZCB0dXJwaXMgdGluY2lkdW50IGlkIGFsaXF1ZXQgcmlzdXMuIEVnZXQgbWF1cmlzIHBoYXJldHJhIGV0IHVsdHJpY2VzIG5lcXVlIG9ybmFyZSBhZW5lYW4gZXVpc21vZC4gRGlhbSBxdWlzIGVuaW0gbG9ib3J0aXMgc2NlbGVyaXNxdWUgZmVybWVudHVtLiBWYXJpdXMgZHVpcyBhdCBjb25zZWN0ZXR1ciBsb3JlbSBkb25lYyBtYXNzYSBzYXBpZW4uIERpYW0gc2l0IGFtZXQgbmlzbCBzdXNjaXBpdCBhZGlwaXNjaW5nIGJpYmVuZHVtIGVzdCB1bHRyaWNpZXMgaW50ZWdlci4gTGVjdHVzIHVybmEgZHVpcyBjb252YWxsaXMgY29udmFsbGlzIHRlbGx1cy4gTmliaCBpcHN1bSBjb25zZXF1YXQgbmlzbCB2ZWwgcHJldGl1bSBsZWN0dXMgcXVhbSBpZCBsZW8uIEZldWdpYXQgaW4gYW50ZSBtZXR1cyBkaWN0dW0gYXQgdGVtcG9yIGNvbW1vZG8uIFZlbGl0IGRpZ25pc3NpbSBzb2RhbGVzIHV0IGV1IHNlbSBpbnRlZ2VyLiBEaWN0dW0gc2l0IGFtZXQganVzdG8gZG9uZWMuIFNjZWxlcmlzcXVlIG1hdXJpcyBwZWxsZW50ZXNxdWUgcHVsdmluYXIgcGVsbGVudGVzcXVlIGhhYml0YW50IG1vcmJpIHRyaXN0aXF1ZSBzZW5lY3R1cy4gTG9yZW0gaXBzdW0gZG9sb3Igc2l0IGFtZXQsIGNvbnNlY3RldHVyIGFkaXBpc2NpbmcgZWxpdCwgc2VkIGRvIGVpdXNtb2QgdGVtcG9yIGluY2lkaWR1bnQgdXQgbGFib3JlIGV0IGRvbG9yZSBtYWduYSBhbGlxdWEuIFNlZCB0dXJwaXMgdGluY2lkdW50IGlkIGFsaXF1ZXQgcmlzdXMuIEVnZXQgbWF1cmlzIHBoYXJldHJhIGV0IHVsdHJpY2VzIG5lcXVlIG9ybmFyZSBhZW5lYW4gZXVpc21vZC4gRGlhbSBxdWlzIGVuaW0gbG9ib3J0aXMgc2NlbGVyaXNxdWUgZmVybWVudHVtLiBWYXJpdXMgZHVpcyBhdCBjb25zZWN0ZXR1ciBsb3JlbSBkb25lYyBtYXNzYSBzYXBpZW4uIERpYW0gc2l0IGFtZXQgbmlzbCBzdXNjaXBpdCBhZGlwaXNjaW5nIGJpYmVuZHVtIGVzdCB1bHRyaWNpZXMgaW50ZWdlci4gTGVjdHVzIHVybmEgZHVpcyBjb252YWxsaXMgY29udmFsbGlzIHRlbGx1cy4gTmliaCBpcHN1bSBjb25zZXF1YXQgbmlzbCB2ZWwgcHJldGl1bSBsZWN0dXMgcXVhbSBpZCBsZW8uIEZldWdpYXQgaW4gYW50ZSBtZXR1cyBkaWN0dW0gYXQgdGVtcG9yIGNvbW1vZG8uIFZlbGl0IGRpZ25pc3NpbSBzb2RhbGVzIHV0IGV1IHNlbSBpbnRlZ2VyLiBEaWN0dW0gc2l0IGFtZXQganVzdG8gZG9uZWMuIFNjZWxlcmlzcXVlIG1hdXJpcyBwZWxsZW50ZXNxdWUgcHVsdmluYXIgcGVsbGVudGVzcXVlIGhhYml0YW50IG1vcmJpIHRyaXN0aXF1ZSBzZW5lY3R1cy5Mb3JlbSBpcHN1bSBkb2xvciBzaXQgYW1ldCwgY29uc2VjdGV0dXIgYWRpcGlzY2luZyBlbGl0LCBzZWQgZG8gZWl1c21vZCB0ZW1wb3IgaW5jaWRpZHVudCB1dCBsYWJvcmUgZXQgZG9sb3JlIG1hZ25hIGFsaXF1YS4gU2VkIHR1cnBpcyB0aW5jaWR1bnQgaWQgYWxpcXVldCByaXN1cy4gRWdldCBtYXVyaXMgcGhhcmV0cmEgZXQgdWx0cmljZXMgbmVxdWUgb3JuYXJlIGFlbmVhbiBldWlzbW9kLiBEaWFtIHF1aXMgZW5pbSBsb2JvcnRpcyBzY2VsZXJpc3F1ZSBmZXJtZW50dW0uIFZhcml1cyBkdWlzIGF0IGNvbnNlY3RldHVyIGxvcmVtIGRvbmVjIG1hc3NhIHNhcGllbi4gRGlhbSBzaXQgYW1ldCBuaXNsIHN1c2NpcGl0IGFkaXBpc2NpbmcgYmliZW5kdW0gZXN0IHVsdHJpY2llcyBpbnRlZ2VyLiBMZWN0dXMgdXJuYSBkdWlzIGNvbnZhbGxpcyBjb252YWxsaXMgdGVsbHVzLiBOaWJoIGlwc3VtIGNvbnNlcXVhdCBuaXNsIHZlbCBwcmV0aXVtIGxlY3R1cyBxdWFtIGlkIGxlby4gRmV1Z2lhdCBpbiBhbnRlIG1ldHVzIGRpY3R1bSBhdCB0ZW1wb3IgY29tbW9kby4gVmVsaXQgZGlnbmlzc2ltIHNvZGFsZXMgdXQgZXUgc2VtIGludGVnZXIuIERpY3R1bSBzaXQgYW1ldCBqdXN0byBkb25lYy4gU2NlbGVyaXNxdWUgbWF1cmlzIHBlbGxlbnRlc3F1ZSBwdWx2aW5hciBwZWxsZW50ZXNxdWUgaGFiaXRhbnQgbW9yYmkgdHJpc3RpcXVlIHNlbmVjdHVzLiBMb3JlbSBpcHN1bSBkb2xvciBzaXQgYW1ldCwgY29uc2VjdGV0dXIgYWRpcGlzY2luZyBlbGl0LCBzZWQgZG8gZWl1c21vZCB0ZW1wb3IgaW5jaWRpZHVudCB1dCBsYWJvcmUgZXQgZG9sb3JlIG1hZ25hIGFsaXF1YS4gU2VkIHR1cnBpcyB0aW5jaWR1bnQgaWQgYWxpcXVldCByaXN1cy4gRWdldCBtYXVyaXMgcGhhcmV0cmEgZXQgdWx0cmljZXMgbmVxdWUgb3JuYXJlIGFlbmVhbiBldWlzbW9kLiBEaWFtIHF1aXMgZW5pbSBsb2JvcnRpcyBzY2VsZXJpc3F1ZSBmZXJtZW50dW0uIFZhcml1cyBkdWlzIGF0IGNvbnNlY3RldHVyIGxvcmVtIGRvbmVjIG1hc3NhIHNhcGllbi4gRGlhbSBzaXQgYW1ldCBuaXNsIHN1c2NpcGl0IGFkaXBpc2NpbmcgYmliZW5kdW0gZXN0IHVsdHJpY2llcyBpbnRlZ2VyLiBMZWN0dXMgdXJuYSBkdWlzIGNvbnZhbGxpcyBjb252YWxsaXMgdGVsbHVzLiBOaWJoIGlwc3VtIGNvbnNlcXVhdCBuaXNsIHZlbCBwcmV0aXVtIGxlY3R1cyBxdWFtIGlkIGxlby4gRmV1Z2lhdCBpbiBhbnRlIG1ldHVzIGRpY3R1bSBhdCB0ZW1wb3IgY29tbW9kby4gVmVsaXQgZGlnbmlzc2ltIHNvZGFsZXMgdXQgZXUgc2VtIGludGVnZXIuIERpY3R1bSBzaXQgYW1ldCBqdXN0byBkb25lYy4gU2NlbGVyaXNxdWUgbWF1cmlzIHBlbGxlbnRlc3F1ZSBwdWx2aW5hciBwZWxsZW50ZXNxdWUgaGFiaXRhbnQgbW9yYmkgdHJpc3RpcXVlIHNlbmVjdHVzLiBTY2VsZXJpc3F1ZSBtYXVyaXMgcGVsbGVudGVzcXVlIHB1bHZpbmFyIHBlbGxlbnRlc3F1ZSBoYWJpdGFudCBtb3JiaSB0cmlzdGlxdWUgc2VuZWN0dXMuIFNjZWxlcmlzcXVlIG1hdXJpcyBwZWxsZW50ZXNxdWUgcHVsdmluYXIgcGVsbGVudGVzcXVlIGhhYml0YW50IG1vcmJpIHRyaXN0aXF1ZSBzZW5lY3R1cy4gU2NlbGVyaXNxdWUgbWF1cmlzIHBlbGxlbnRlc3F1ZSBwdWx2aW5hciBwZWxsZW50ZXNxdWUgaGFiaXRhbnQgbW9yYmkgdHJpc3RpcXVlIHNlbmVjdHVzLiBTY2VsZXJpc3F1ZSBtYXVyaXMgcGVsbGVudGVzcXVlIHB1bHZpbmFyIHBlbGxlbnRlc3F1ZSBoYWJpdGFudCBtb3JiaSB0cmlzdGlxdWUgc2VuZWN0dXMuIFNjZWxlcmlzcXVlIG1hdXJpcyBwZWxsZW50ZXNxdWUgcHVsdmluYXIgcGVsbGVudGVzcXVlIGhhYml0YW50IG1vcmJpIHRyaXN0aXF1ZSBzZW5lY3R1cy4=") for i in std.range(0,100)
    ],
}
```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 45.7 ± 1.4 | 43.6 | 52.1 | 1.00 |
| `Go` | 303.6 ± 8.4 | 292.1 | 324.7 | 6.64 ± 0.27 |
| `Scala` | 406.3 ± 8.3 | 397.0 | 424.8 | 8.88 ± 0.32 |
| `C++` | 10058.3 ± 337.9 | 9738.8 | 10641.5 | 219.86 ± 9.92 |

### std.base64 (byte array)

<details>
<summary>Source</summary>

```jsonnet
{
    foo: [
        std.base64([76,111,114,101,109,32,105,112,115,117,109,32,100,111,108,111,114,32,115,105,116,32,97,109,101,116,44,32,99,111,110,115,101,99,116,101,116,117,114,32,97,100,105,112,105,115,99,105,110,103,32,101,108,105,116,44,32,115,101,100,32,100,111,32,101,105,117,115,109,111,100,32,116,101,109,112,111,114,32,105,110,99,105,100,105,100,117,110,116,32,117,116,32,108,97,98,111,114,101,32,101,116,32,100,111,108,111,114,101,32,109,97,103,110,97,32,97,108,105,113,117,97,46,32,83,101,100,32,116,117,114,112,105,115,32,116,105,110,99,105,100,117,110,116,32,105,100,32,97,108,105,113,117,101,116,32,114,105,115,117,115,46,32,69,103,101,116,32,109,97,117,114,105,115,32,112,104,97,114,101,116,114,97,32,101,116,32,117,108,116,114,105,99,101,115,32,110,101,113,117,101,32,111,114,110,97,114,101,32,97,101,110,101,97,110,32,101,117,105,115,109,111,100,46,32,68,105,97,109,32,113,117,105,115,32,101,110,105,109,32,108,111,98,111,114,116,105,115,32,115,99,101,108,101,114,105,115,113,117,101,32,102,101,114,109,101,110,116,117,109,46,32,86,97,114,105,117,115,32,100,117,105,115,32,97,116,32,99,111,110,115,101,99,116,101,116,117,114,32,108,111,114,101,109,32,100,111,110,101,99,32,109,97,115,115,97,32,115,97,112,105,101,110,46,32,68,105,97,109,32,115,105,116,32,97,109,101,116,32,110,105,115,108,32,115,117,115,99,105,112,105,116,32,97,100,105,112,105,115,99,105,110,103,32,98,105,98,101,110,100,117,109,32,101,115,116,32,117,108,116,114,105,99,105,101,115,32,105,110,116,101,103,101,114,46,32,76,101,99,116,117,115,32,117,114,110,97,32,100,117,105,115,32,99,111,110,118,97,108,108,105,115,32,99,111,110,118,97,108,108,105,115,32,116,101,108,108,117,115,46,32,78,105,98,104,32,105,112,115,117,109,32,99,111,110,115,101,113,117,97,116,32,110,105,115,108,32,118,101,108,32,112,114,101,116,105,117,109,32,108,101,99,116,117,115,32,113,117,97,109,32,105,100,32,108,101,111,46,32,70,101,117,103,105,97,116,32,105,110,32,97,110,116,101,32,109,101,116,117,115,32,100,105,99,116,117,109,32,97,116,32,116,101,109,112,111,114,32,99,111,109,109,111,100,111,46,32,86,101,108,105,116,32,100,105,103,110,105,115,115,105,109,32,115,111,100,97,108,101,115,32,117,116,32,101,117,32,115,101,109,32,105,110,116,101,103,101,114,46,32,68,105,99,116,117,109,32,115,105,116,32,97,109,101,116,32,106,117,115,116,111,32,100,111,110,101,99,46,32,83,99,101,108,101,114,105,115,113,117,101,32,109,97,117,114,105,115,32,112,101,108,108,101,110,116,101,115,113,117,101,32,112,117,108,118,105,110,97,114,32,112,101,108,108,101,110,116,101,115,113,117,101,32,104,97,98,105,116,97,110,116,32,109,111,114,98,105,32,116,114,105,115,116,105,113,117,101,32,115,101,110,101,99,116,117,115,46,32,76,111,114,101,109,32,105,112,115,117,109,32,100,111,108,111,114,32,115,105,116,32,97,109,101,116,44,32,99,111,110,115,101,99,116,101,116,117,114,32,97,100,105,112,105,115,99,105,110,103,32,101,108,105,116,44,32,115,101,100,32,100,111,32,101,105,117,115,109,111,100,32,116,101,109,112,111,114,32,105,110,99,105,100,105,100,117,110,116,32,117,116,32,108,97,98,111,114,101,32,101,116,32,100,111,108,111,114,101,32,109,97,103,110,97,32,97,108,105,113,117,97,46,32,83,101,100,32,116,117,114,112,105,115,32,116,105,110,99,105,100,117,110,116,32,105,100,32,97,108,105,113,117,101,116,32,114,105,115,117,115,46,32,69,103,101,116,32,109,97,117,114,105,115,32,112,104,97,114,101,116,114,97,32,101,116,32,117,108,116,114,105,99,101,115,32,110,101,113,117,101,32,111,114,110,97,114,101,32,97,101,110,101,97,110,32,101,117,105,115,109,111,100,46,32,68,105,97,109,32,113,117,105,115,32,101,110,105,109,32,108,111,98,111,114,116,105,115,32,115,99,101,108,101,114,105,115,113,117,101,32,102,101,114,109,101,110,116,117,109,46,32,86,97,114,105,117,115,32,100,117,105,115,32,97,116,32,99,111,110,115,101,99,116,101,116,117,114,32,108,111,114,101,109,32,100,111,110,101,99,32,109,97,115,115,97,32,115,97,112,105,101,110,46,32,68,105,97,109,32,115,105,116,32,97,109,101,116,32,110,105,115,108,32,115,117,115,99,105,112,105,116,32,97,100,105,112,105,115,99,105,110,103,32,98,105,98,101,110,100,117,109,32,101,115,116,32,117,108,116,114,105,99,105,101,115,32,105,110,116,101,103,101,114,46,32,76,101,99,116,117,115,32,117,114,110,97,32,100,117,105,115,32,99,111,110,118,97,108,108,105,115,32,99,111,110,118,97,108,108,105,115,32,116,101,108,108,117,115,46,32,78,105,98,104,32,105,112,115,117,109,32,99,111,110,115,101,113,117,97,116,32,110,105,115,108,32,118,101,108,32,112,114,101,116,105,117,109,32,108,101,99,116,117,115,32,113,117,97,109,32,105,100,32,108,101,111,46,32,70,101,117,103,105,97,116,32,105,110,32,97,110,116,101,32,109,101,116,117,115,32,100,105,99,116,117,109,32,97,116,32,116,101,109,112,111,114,32,99,111,109,109,111,100,111,46,32,86,101,108,105,116,32,100,105,103,110,105,115,115,105,109,32,115,111,100,97,108,101,115,32,117,116,32,101,117,32,115,101,109,32,105,110,116,101,103,101,114,46,32,68,105,99,116,117,109,32,115,105,116,32,97,109,101,116,32,106,117,115,116,111,32,100,111,110,101,99,46,32,83,99,101,108,101,114,105,115,113,117,101,32,109,97,117,114,105,115,32,112,101,108,108,101,110,116,101,115,113,117,101,32,112,117,108,118,105,110,97,114,32,112,101,108,108,101,110,116,101,115,113,117,101,32,104,97,98,105,116,97,110,116,32,109,111,114,98,105,32,116,114,105,115,116,105,113,117,101,32,115,101,110,101,99,116,117,115,46,32,76,111,114,101,109,32,105,112,115,117,109,32,100,111,108,111,114,32,115,105,116,32,97,109,101,116,44,32,99,111,110,115,101,99,116,101,116,117,114,32,97,100,105,112,105,115,99,105,110,103,32,101,108,105,116,44,32,115,101,100,32,100,111,32,101,105,117,115,109,111,100,32,116,101,109,112,111,114,32,105,110,99,105,100,105,100,117,110,116,32,117,116,32,108,97,98,111,114,101,32,101,116,32,100,111,108,111,114,101,32,109,97,103,110,97,32,97,108,105,113,117,97,46,32,83,101,100,32,116,117,114,112,105,115,32,116,105,110,99,105,100,117,110,116,32,105,100,32,97,108,105,113,117,101,116,32,114,105,115,117,115,46,32,69,103,101,116,32,109,97,117,114,105,115,32,112,104,97,114,101,116,114,97,32,101,116,32,117,108,116,114,105,99,101,115,32,110,101,113,117,101,32,111,114,110,97,114,101,32,97,101,110,101,97,110,32,101,117,105,115,109,111,100,46,32,68,105,97,109,32,113,117,105,115,32,101,110,105,109,32,108,111,98,111,114,116,105,115,32,115,99,101,108,101,114,105,115,113,117,101,32,102,101,114,109,101,110,116,117,109,46,32,86,97,114,105,117,115,32,100,117,105,115,32,97,116,32,99,111,110,115,101,99,116,101,116,117,114,32,108,111,114,101,109,32,100,111,110,101,99,32,109,97,115,115,97,32,115,97,112,105,101,110,46,32,68,105,97,109,32,115,105,116,32,97,109,101,116,32,110,105,115,108,32,115,117,115,99,105,112,105,116,32,97,100,105,112,105,115,99,105,110,103,32,98,105,98,101,110,100,117,109,32,101,115,116,32,117,108,116,114,105,99,105,101,115,32,105,110,116,101,103,101,114,46,32,76,101,99,116,117,115,32,117,114,110,97,32,100,117,105,115,32,99,111,110,118,97,108,108,105,115,32,99,111,110,118,97,108,108,105,115,32,116,101,108,108,117,115,46,32,78,105,98,104,32,105,112,115,117,109,32,99,111,110,115,101,113,117,97,116,32,110,105,115,108,32,118,101,108,32,112,114,101,116,105,117,109,32,108,101,99,116,117,115,32,113,117,97,109,32,105,100,32,108,101,111,46,32,70,101,117,103,105,97,116,32,105,110,32,97,110,116,101,32,109,101,116,117,115,32,100,105,99,116,117,109,32,97,116,32,116,101,109,112,111,114,32,99,111,109,109,111,100,111,46,32,86,101,108,105,116,32,100,105,103,110,105,115,115,105,109,32,115,111,100,97,108,101,115,32,117,116,32,101,117,32,115,101,109,32,105,110,116,101,103,101,114,46,32,68,105,99,116,117,109,32,115,105,116,32,97,109,101,116,32,106,117,115,116,111,32,100,111,110,101,99,46,32,83,99,101,108,101,114,105,115,113,117,101,32,109,97,117,114,105,115,32,112,101,108,108,101,110,116,101,115,113,117,101,32,112,117,108,118,105,110,97,114,32,112,101,108,108,101,110,116,101,115,113,117,101,32,104,97,98,105,116,97,110,116,32,109,111,114,98,105,32,116,114,105,115,116,105,113,117,101,32,115,101,110,101,99,116,117,115,46,76,111,114,101,109,32,105,112,115,117,109,32,100,111,108,111,114,32,115,105,116,32,97,109,101,116,44,32,99,111,110,115,101,99,116,101,116,117,114,32,97,100,105,112,105,115,99,105,110,103,32,101,108,105,116,44,32,115,101,100,32,100,111,32,101,105,117,115,109,111,100,32,116,101,109,112,111,114,32,105,110,99,105,100,105,100,117,110,116,32,117,116,32,108,97,98,111,114,101,32,101,116,32,100,111,108,111,114,101,32,109,97,103,110,97,32,97,108,105,113,117,97,46,32,83,101,100,32,116,117,114,112,105,115,32,116,105,110,99,105,100,117,110,116,32,105,100,32,97,108,105,113,117,101,116,32,114,105,115,117,115,46,32,69,103,101,116,32,109,97,117,114,105,115,32,112,104,97,114,101,116,114,97,32,101,116,32,117,108,116,114,105,99,101,115,32,110,101,113,117,101,32,111,114,110,97,114,101,32,97,101,110,101,97,110,32,101,117,105,115,109,111,100,46,32,68,105,97,109,32,113,117,105,115,32,101,110,105,109,32,108,111,98,111,114,116,105,115,32,115,99,101,108,101,114,105,115,113,117,101,32,102,101,114,109,101,110,116,117,109,46,32,86,97,114,105,117,115,32,100,117,105,115,32,97,116,32,99,111,110,115,101,99,116,101,116,117,114,32,108,111,114,101,109,32,100,111,110,101,99,32,109,97,115,115,97,32,115,97,112,105,101,110,46,32,68,105,97,109,32,115,105,116,32,97,109,101,116,32,110,105,115,108,32,115,117,115,99,105,112,105,116,32,97,100,105,112,105,115,99,105,110,103,32,98,105,98,101,110,100,117,109,32,101,115,116,32,117,108,116,114,105,99,105,101,115,32,105,110,116,101,103,101,114,46,32,76,101,99,116,117,115,32,117,114,110,97,32,100,117,105,115,32,99,111,110,118,97,108,108,105,115,32,99,111,110,118,97,108,108,105,115,32,116,101,108,108,117,115,46,32,78,105,98,104,32,105,112,115,117,109,32,99,111,110,115,101,113,117,97,116,32,110,105,115,108,32,118,101,108,32,112,114,101,116,105,117,109,32,108,101,99,116,117,115,32,113,117,97,109,32,105,100,32,108,101,111,46,32,70,101,117,103,105,97,116,32,105,110,32,97,110,116,101,32,109,101,116,117,115,32,100,105,99,116,117,109,32,97,116,32,116,101,109,112,111,114,32,99,111,109,109,111,100,111,46,32,86,101,108,105,116,32,100,105,103,110,105,115,115,105,109,32,115,111,100,97,108,101,115,32,117,116,32,101,117,32,115,101,109,32,105,110,116,101,103,101,114,46,32,68,105,99,116,117,109,32,115,105,116,32,97,109,101,116,32,106,117,115,116,111,32,100,111,110,101,99,46,32,83,99,101,108,101,114,105,115,113,117,101,32,109,97,117,114,105,115,32,112,101,108,108,101,110,116,101,115,113,117,101,32,112,117,108,118,105,110,97,114,32,112,101,108,108,101,110,116,101,115,113,117,101,32,104,97,98,105,116,97,110,116,32,109,111,114,98,105,32,116,114,105,115,116,105,113,117,101,32,115,101,110,101,99,116,117,115,46,32,76,111,114,101,109,32,105,112,115,117,109,32,100,111,108,111,114,32,115,105,116,32,97,109,101,116,44,32,99,111,110,115,101,99,116,101,116,117,114,32,97,100,105,112,105,115,99,105,110,103,32,101,108,105,116,44,32,115,101,100,32,100,111,32,101,105,117,115,109,111,100,32,116,101,109,112,111,114,32,105,110,99,105,100,105,100,117,110,116,32,117,116,32,108,97,98,111,114,101,32,101,116,32,100,111,108,111,114,101,32,109,97,103,110,97,32,97,108,105,113,117,97,46,32,83,101,100,32,116,117,114,112,105,115,32,116,105,110,99,105,100,117,110,116,32,105,100,32,97,108,105,113,117,101,116,32,114,105,115,117,115,46,32,69,103,101,116,32,109,97,117,114,105,115,32,112,104,97,114,101,116,114,97,32,101,116,32,117,108,116,114,105,99,101,115,32,110,101,113,117,101,32,111,114,110,97,114,101,32,97,101,110,101,97,110,32,101,117,105,115,109,111,100,46,32,68,105,97,109,32,113,117,105,115,32,101,110,105,109,32,108,111,98,111,114,116,105,115,32,115,99,101,108,101,114,105,115,113,117,101,32,102,101,114,109,101,110,116,117,109,46,32,86,97,114,105,117,115,32,100,117,105,115,32,97,116,32,99,111,110,115,101,99,116,101,116,117,114,32,108,111,114,101,109,32,100,111,110,101,99,32,109,97,115,115,97,32,115,97,112,105,101,110,46,32,68,105,97,109,32,115,105,116,32,97,109,101,116,32,110,105,115,108,32,115,117,115,99,105,112,105,116,32,97,100,105,112,105,115,99,105,110,103,32,98,105,98,101,110,100,117,109,32,101,115,116,32,117,108,116,114,105,99,105,101,115,32,105,110,116,101,103,101,114,46,32,76,101,99,116,117,115,32,117,114,110,97,32,100,117,105,115,32,99,111,110,118,97,108,108,105,115,32,99,111,110,118,97,108,108,105,115,32,116,101,108,108,117,115,46,32,78,105,98,104,32,105,112,115,117,109,32,99,111,110,115,101,113,117,97,116,32,110,105,115,108,32,118,101,108,32,112,114,101,116,105,117,109,32,108,101,99,116,117,115,32,113,117,97,109,32,105,100,32,108,101,111,46,32,70,101,117,103,105,97,116,32,105,110,32,97,110,116,101,32,109,101,116,117,115,32,100,105,99,116,117,109,32,97,116,32,116,101,109,112,111,114,32,99,111,109,109,111,100,111,46,32,86,101,108,105,116,32,100,105,103,110,105,115,115,105,109,32,115,111,100,97,108,101,115,32,117,116,32,101,117,32,115,101,109,32,105,110,116,101,103,101,114,46,32,68,105,99,116,117,109,32,115,105,116,32,97,109,101,116,32,106,117,115,116,111,32,100,111,110,101,99,46,32,83,99,101,108,101,114,105,115,113,117,101,32,109,97,117,114,105,115,32,112,101,108,108,101,110,116,101,115,113,117,101,32,112,117,108,118,105,110,97,114,32,112,101,108,108,101,110,116,101,115,113,117,101,32,104,97,98,105,116,97,110,116,32,109,111,114,98,105,32,116,114,105,115,116,105,113,117,101,32,115,101,110,101,99,116,117,115,46,32,83,99,101,108,101,114,105,115,113,117,101,32,109,97,117,114,105,115,32,112,101,108,108,101,110,116,101,115,113,117,101,32,112,117,108,118,105,110,97,114,32,112,101,108,108,101,110,116,101,115,113,117,101,32,104,97,98,105,116,97,110,116,32,109,111,114,98,105,32,116,114,105,115,116,105,113,117,101,32,115,101,110,101,99,116,117,115,46,32,83,99,101,108,101,114,105,115,113,117,101,32,109,97,117,114,105,115,32,112,101,108,108,101,110,116,101,115,113,117,101,32,112,117,108,118,105,110,97,114,32,112,101,108,108,101,110,116,101,115,113,117,101,32,104,97,98,105,116,97,110,116,32,109,111,114,98,105,32,116,114,105,115,116,105,113,117,101,32,115,101,110,101,99,116,117,115,46,32,83,99,101,108,101,114,105,115,113,117,101,32,109,97,117,114,105,115,32,112,101,108,108,101,110,116,101,115,113,117,101,32,112,117,108,118,105,110,97,114,32,112,101,108,108,101,110,116,101,115,113,117,101,32,104,97,98,105,116,97,110,116,32,109,111,114,98,105,32,116,114,105,115,116,105,113,117,101,32,115,101,110,101,99,116,117,115,46,32,83,99,101,108,101,114,105,115,113,117,101,32,109,97,117,114,105,115,32,112,101,108,108,101,110,116,101,115,113,117,101,32,112,117,108,118,105,110,97,114,32,112,101,108,108,101,110,116,101,115,113,117,101,32,104,97,98,105,116,97,110,116,32,109,111,114,98,105,32,116,114,105,115,116,105,113,117,101,32,115,101,110,101,99,116,117,115,46,32,83,99,101,108,101,114,105,115,113,117,101,32,109,97,117,114,105,115,32,112,101,108,108,101,110,116,101,115,113,117,101,32,112,117,108,118,105,110,97,114,32,112,101,108,108,101,110,116,101,115,113,117,101,32,104,97,98,105,116,97,110,116,32,109,111,114,98,105,32,116,114,105,115,116,105,113,117,101,32,115,101,110,101,99,116,117,115]),for i in std.range(0,100)
    ],
}
```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 37.0 ± 1.8 | 33.7 | 42.5 | 1.00 |
| `Go` | 195.8 ± 11.5 | 180.9 | 219.5 | 5.28 ± 0.41 |
| `Scala` | 419.2 ± 13.0 | 396.5 | 454.6 | 11.32 ± 0.66 |
| `C++` | 15605.3 ± 310.2 | 15173.1 | 16065.6 | 421.22 ± 22.47 |

### std.foldl

<details>
<summary>Source</summary>

```jsonnet
local input = std.makeArray(10000, function(i) 'xxxxx');

std.foldl(function(acc, value) acc + value, input, '')

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 7.7 ± 0.7 | 6.8 | 18.5 | 1.00 |
| `Go` | 32.6 ± 1.1 | 30.6 | 37.8 | 4.26 ± 0.41 |
| `Scala` | 461.7 ± 8.5 | 445.0 | 484.5 | 60.35 ± 5.54 |
| `C++` | 320.3 ± 6.0 | 308.6 | 335.0 | 41.87 ± 3.85 |

### std.manifestJsonEx

<details>
<summary>Source</summary>

```jsonnet
{
  bar: {
    prometheusOperator+: {
      service+: {
        spec+: {
          ports: [
            {
              name: 'https',
              port: 8443,
              targetPort: 'https',
            },
          ],
        },
      },
      serviceMonitor+: {
        spec+: {
          endpoints: [
            {
              port: 'https',
              scheme: 'https',
              honorLabels: true,
              bearerTokenFile: '/var/run/secrets/kubernetes.io/serviceaccount/token',
              tlsConfig: {
                insecureSkipVerify: true,
              },
            },
          ]
        },
      },
      clusterRole+: {
        rules+: [
          {
            apiGroups: ['authentication.k8s.io'],
            resources: ['tokenreviews'],
            verbs: ['create'],
          },
          {
            apiGroups: ['authorization.k8s.io'],
            resources: ['subjectaccessreviews'],
            verbs: ['create'],
          },
        ],
      },
    }
  },
  foo: std.manifestJsonEx(self.bar, "  ")
}
```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 1.9 ± 0.1 | 1.7 | 3.0 | 1.00 |
| `Go` | 4.2 ± 0.3 | 3.6 | 8.0 | 2.14 ± 0.18 |
| `Scala` | 338.9 ± 3.4 | 334.9 | 348.8 | 174.49 ± 8.63 |
| `C++` | 106.6 ± 3.2 | 102.8 | 113.2 | 54.90 ± 3.12 |

### std.manifestTomlEx

> Note: No results for Scala, std.manifestTomlEx is not implemented

<details>
<summary>Source</summary>

```jsonnet
{
  bar: {
    prometheusOperator+: {
      service+: {
        spec+: {
          ports: [
            {
              name: 'https',
              port: 8443,
              targetPort: 'https',
            },
          ],
        },
      },
      serviceMonitor+: {
        spec+: {
          endpoints: [
            {
              port: 'https',
              scheme: 'https',
              honorLabels: true,
              bearerTokenFile: '/var/run/secrets/kubernetes.io/serviceaccount/token',
              tlsConfig: {
                insecureSkipVerify: true,
              },
            },
          ],
        },
      },
      clusterRole+: {
        rules+: [
          {
            apiGroups: ['authentication.k8s.io'],
            resources: ['tokenreviews'],
            verbs: ['create'],
          },
          {
            apiGroups: ['authorization.k8s.io'],
            resources: ['subjectaccessreviews'],
            verbs: ['create'],
          },
        ],
      },
    },
  },
  nothing: std.manifestTomlEx(self.bar, '  '),
}

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 1.9 ± 0.1 | 1.8 | 2.7 | 1.00 |
| `Go` | 4.2 ± 0.3 | 3.7 | 8.0 | 2.17 ± 0.19 |
| `C++` | 1131.6 ± 28.1 | 1089.2 | 1198.7 | 584.01 ± 30.62 |

### std.parseInt

<details>
<summary>Source</summary>

```jsonnet
{
    foo: [
        std.parseInt("-123949595") for i in std.range(0,100)
    ],
}
```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 2.0 ± 0.1 | 1.7 | 3.2 | 1.00 |
| `Go` | 3.7 ± 0.4 | 3.1 | 9.0 | 1.85 ± 0.21 |
| `Scala` | 332.3 ± 7.2 | 326.1 | 359.7 | 168.32 ± 10.37 |
| `C++` | 110.2 ± 1.9 | 106.9 | 115.1 | 55.82 ± 3.36 |

### std.reverse

> Note: No results for Scala, std.reverse is not implemented

<details>
<summary>Source</summary>

```jsonnet
{
    foo: [
        std.reverse(std.range(0, 5000)) for i in std.range(0,100)
    ],
}
```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 59.7 ± 1.9 | 56.3 | 64.7 | 1.00 |
| `Go` | 363.6 ± 9.0 | 352.8 | 385.1 | 6.09 ± 0.25 |
| `C++` | 865.9 ± 16.1 | 839.9 | 902.0 | 14.51 ± 0.54 |

### std.substr

<details>
<summary>Source</summary>

```jsonnet
{
    foo: [
        std.substr("Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Sed turpis tincidunt id aliquet risus. Eget mauris pharetra et ultrices neque ornare aenean euismod. Diam quis enim lobortis scelerisque fermentum. Varius duis at consectetur lorem donec massa sapien. Diam sit amet nisl suscipit adipiscing bibendum est ultricies integer. Lectus urna duis convallis convallis tellus. Nibh ipsum consequat nisl vel pretium lectus quam id leo. Feugiat in ante metus dictum at tempor commodo. Velit dignissim sodales ut eu sem integer. Dictum sit amet justo donec. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Sed turpis tincidunt id aliquet risus. Eget mauris pharetra et ultrices neque ornare aenean euismod. Diam quis enim lobortis scelerisque fermentum. Varius duis at consectetur lorem donec massa sapien. Diam sit amet nisl suscipit adipiscing bibendum est ultricies integer. Lectus urna duis convallis convallis tellus. Nibh ipsum consequat nisl vel pretium lectus quam id leo. Feugiat in ante metus dictum at tempor commodo. Velit dignissim sodales ut eu sem integer. Dictum sit amet justo donec. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Sed turpis tincidunt id aliquet risus. Eget mauris pharetra et ultrices neque ornare aenean euismod. Diam quis enim lobortis scelerisque fermentum. Varius duis at consectetur lorem donec massa sapien. Diam sit amet nisl suscipit adipiscing bibendum est ultricies integer. Lectus urna duis convallis convallis tellus. Nibh ipsum consequat nisl vel pretium lectus quam id leo. Feugiat in ante metus dictum at tempor commodo. Velit dignissim sodales ut eu sem integer. Dictum sit amet justo donec. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus.Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Sed turpis tincidunt id aliquet risus. Eget mauris pharetra et ultrices neque ornare aenean euismod. Diam quis enim lobortis scelerisque fermentum. Varius duis at consectetur lorem donec massa sapien. Diam sit amet nisl suscipit adipiscing bibendum est ultricies integer. Lectus urna duis convallis convallis tellus. Nibh ipsum consequat nisl vel pretium lectus quam id leo. Feugiat in ante metus dictum at tempor commodo. Velit dignissim sodales ut eu sem integer. Dictum sit amet justo donec. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Sed turpis tincidunt id aliquet risus. Eget mauris pharetra et ultrices neque ornare aenean euismod. Diam quis enim lobortis scelerisque fermentum. Varius duis at consectetur lorem donec massa sapien. Diam sit amet nisl suscipit adipiscing bibendum est ultricies integer. Lectus urna duis convallis convallis tellus. Nibh ipsum consequat nisl vel pretium lectus quam id leo. Feugiat in ante metus dictum at tempor commodo. Velit dignissim sodales ut eu sem integer. Dictum sit amet justo donec. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. Scelerisque mauris pellentesque pulvinar pellentesque habitant morbi tristique senectus. ", i, 800-i) for i in std.range(0,100)
    ],
}
```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 2.6 ± 0.1 | 2.3 | 3.5 | 1.00 |
| `Go` | 8.9 ± 0.6 | 8.0 | 15.3 | 3.46 ± 0.29 |
| `Scala` | 346.6 ± 9.4 | 337.8 | 375.9 | 134.32 ± 7.51 |
| `C++` | 31.4 ± 1.0 | 29.7 | 36.4 | 12.17 ± 0.72 |

### Comparsion for array

> Note: No results for Scala, array comparsion is not implemented

<details>
<summary>Source</summary>

```jsonnet
local long_array = std.range(1, 1000000);
long_array + [1] < long_array + [2]

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 26.0 ± 0.6 | 25.4 | 30.2 | 1.00 |
| `Go` | 121.5 ± 6.5 | 116.2 | 141.0 | 4.68 ± 0.28 |
| `C++` | 147098.5 ± 3334.1 | 135178.4 | 150470.0 | 5663.91 ± 187.03 |

### Comparsion for primitives

> Note: No results for C++, can't run: uses up to 192GB of RAM

<details>
<summary>Source</summary>

```jsonnet
([ i < j for i in std.range(1, 1000) for j in std.range(1, 1000)])

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 284.7 ± 10.4 | 267.1 | 315.4 | 1.00 |
| `Go` | 2009.5 ± 46.0 | 1931.3 | 2108.9 | 7.06 ± 0.31 |
| `Scala` | 550.0 ± 14.8 | 526.3 | 575.4 | 1.93 ± 0.09 |
