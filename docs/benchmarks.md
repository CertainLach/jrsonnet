# Benchmarks

There are multiple implementations of jsonnet implemented in different languages: Rust (this repo), [Go](https://github.com/google/go-jsonnet/), [Scala](https://github.com/databricks/sjsonnet), [C++](https://github.com/google/jsonnet), [Haskell](https://github.com/moleike/haskell-jsonnet).

For simplicity, I will call these implementations by the language of their implementation.

Unfortunately, I haven't managed to measure performance of Haskell implementation, because I wasn't able to build it, and there is no binaries published anywhere, so this implementation is omitted from the following benchmarks

<details>
<summary>Tested versions</summary>

Go: Jsonnet commandline interpreter (Go implementation) v0.20.0

```
Jsonnet commandline interpreter (Go implementation) v0.20.0

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

C++: Jsonnet commandline interpreter v0.20.0

```
Jsonnet commandline interpreter v0.20.0

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
Expected Signature: Sjsonnet 0.4.9
usage: sjsonnet [sjsonnet-options] script-file
  -i --interactive                      Run Mill in interactive mode, suitable for opening REPLs and
                                        taking user input
  -J --jpath <str>                      Specify an additional library search dir (right-most wins)
  -o --output-file <str>                Write to the output file rather than stdout
  -m --multi <str>                      Write multiple files to the directory, list files on stdout
  -c --create-output-dirs               Automatically creates all parent directories for files
  -y --yaml-stream                      Write output as a YAML stream of JSON documents
  -S --string                           Expect a string, manifest as plain text
  -V --ext-str <str>                    <var>[=<val>] Provide 'external' variable as string. 'If
                                        <val> is omitted, get from environment var <var>
  --ext-str-file <str>                  <var>=<file> Provide 'external' variable as string from the
                                        file
  -V --ext-code <str>                   <var>[=<code>] Provide 'external' variable as Jsonnet code.
                                        If <code> is omitted, get from environment var <var>
  --ext-code-file <str>                 <var>=<file> Provide 'external' variable as Jsonnet code
                                        from the file
  -A --tla-str <str>                    <var>[=<val>] Provide top-level arguments as string. 'If
                                        <val> is omitted, get from environment var <var>
  --tla-str-file <str>                  <var>=<file> Provide top-level arguments variable as string
                                        from the file
  -V --tla-code <str>                   <var>[=<val>] Provide top-level arguments as Jsonnet code.
                                        'If <val> is omitted, get from environment var <var>
  --tla-code-file <str>                 <var>=<file> Provide top-level arguments variable as Jsonnet
                                        code from the file
  -n --indent <int>                     How much to indent your output JSON
  -p --preserve-order                   Preserves order of keys in the resulting JSON
  --strict                              Enforce some additional syntax limitations
  --yaml-out                            Write output as a YAML document
  file <str>                            The jsonnet file you wish to evaluate
  --yaml-debug                          Generate source line comments in the output YAML doc to make
                                        it easier to figure out where values come from.
  --no-static-errors                    Turn static errors into warnings
  --fatal-warnings                      Fail if any warnings were emitted
  -e --exec                             Evaluate the given string as Jsonnet rather than treating it
                                        as a file name
  --no-duplicate-keys-in-comprehension  Raise an error if an object comprehension contains duplicate
                                        keys
  --strict-import-syntax                Raise an error if import expressions are used without proper
                                        parentheses, e.g. import "foo".bar rather than (import
                                        "foo").bar
  --strict-inherited-assertions         Properly handle assertions defined in a Jsonnet dictionary
                                        that is extended more than once


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
| `Rust` | 119.0 ± 6.1 | 98.2 | 122.8 | 1.00 |
| `Go` | 2033.3 ± 29.6 | 1973.3 | 2088.8 | 17.09 ± 0.91 |
| `Scala` | 801.3 ± 17.5 | 769.1 | 830.1 | 6.73 ± 0.37 |

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
| `Rust` | 150.1 ± 11.3 | 129.9 | 161.3 | 1.00 |
| `Go` | 3221.2 ± 46.4 | 3137.2 | 3272.6 | 21.46 ± 1.64 |
| `Scala` | 1056.3 ± 24.8 | 1006.7 | 1107.6 | 7.04 ± 0.55 |
| `C++` | 90142.7 ± 371.0 | 89516.1 | 91006.4 | 600.66 ± 45.19 |

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
| `Rust` | 9.8 ± 1.3 | 5.8 | 11.8 | 1.00 |
| `Go` | 159.5 ± 10.2 | 139.4 | 175.0 | 16.24 ± 2.32 |
| `Scala` | 388.0 ± 11.8 | 359.7 | 405.5 | 39.50 ± 5.19 |
| `C++` | 96.7 ± 8.8 | 77.3 | 104.9 | 9.84 ± 1.54 |

### Large string template

> Note: No results for Go, fails with os stack size exhausion

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 11.8 ± 1.1 | 7.4 | 14.6 | 1.00 |
| `Scala` | 468.1 ± 9.7 | 452.2 | 494.6 | 39.57 ± 3.82 |
| `C++` | 14893.8 ± 102.4 | 14703.4 | 15074.0 | 1259.12 ± 119.13 |

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
| `Rust` | 23.6 ± 2.3 | 13.6 | 29.5 | 1.00 |
| `Go` | 11709.4 ± 73.0 | 11566.7 | 11825.3 | 496.43 ± 49.03 |
| `Scala` | 446.3 ± 8.9 | 428.7 | 467.4 | 18.92 ± 1.90 |
| `C++` | 24687.7 ± 144.7 | 24488.2 | 24951.9 | 1046.65 ± 103.36 |

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
| `Rust` | 283.0 ± 10.0 | 264.4 | 298.1 | 1.00 |
| `Go` | 10224.6 ± 83.7 | 10015.8 | 10346.7 | 36.13 ± 1.32 |
| `Scala` | 767.9 ± 22.3 | 736.1 | 805.2 | 2.71 ± 0.12 |
| `C++` | 28183.8 ± 403.2 | 27302.6 | 28819.9 | 99.59 ± 3.81 |

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
| `Rust` | 3.0 ± 0.4 | 1.8 | 4.0 | 1.00 |
| `Go` | 8.1 ± 1.4 | 5.0 | 10.3 | 2.70 ± 0.59 |
| `Scala` | 346.8 ± 11.1 | 320.1 | 360.6 | 114.96 ± 15.61 |
| `C++` | 51.7 ± 10.2 | 32.3 | 61.0 | 17.12 ± 4.07 |

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
| `Rust` | 388.9 ± 11.9 | 367.2 | 408.5 | 1.00 |
| `Go` | 1566.1 ± 20.3 | 1532.8 | 1605.8 | 4.03 ± 0.13 |
| `Scala` | 515.8 ± 15.4 | 481.6 | 540.9 | 1.33 ± 0.06 |
| `C++` | 2719.1 ± 22.9 | 2660.2 | 2754.3 | 6.99 ± 0.22 |

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
| `Rust` | 105.0 ± 8.6 | 83.9 | 119.9 | 1.00 |
| `Go` | 666.5 ± 14.3 | 640.1 | 691.4 | 6.35 ± 0.54 |
| `Scala` | 398.7 ± 11.8 | 361.5 | 419.8 | 3.80 ± 0.33 |
| `C++` | 217.0 ± 7.4 | 198.5 | 223.3 | 2.07 ± 0.18 |

### Foldl string concat

<details>
<summary>Source</summary>

```jsonnet
std.foldl(function(e, res) e + res, std.makeArray(20000, function(i) 'aaaaa'), '')

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 16.0 ± 2.8 | 9.2 | 24.2 | 1.00 |
| `Go` | 84.7 ± 6.1 | 68.5 | 98.1 | 5.28 ± 0.98 |
| `Scala` | 591.5 ± 16.2 | 567.5 | 622.1 | 36.86 ± 6.41 |
| `C++` | 901.2 ± 14.6 | 868.4 | 929.5 | 56.16 ± 9.69 |

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
| `Rust` | 5.4 ± 0.7 | 3.1 | 7.5 | 1.00 |
| `Go` | 20.7 ± 2.6 | 12.6 | 24.7 | 3.84 ± 0.66 |
| `C++` | 3826.6 ± 47.9 | 3765.1 | 3946.1 | 707.87 ± 86.40 |

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
| `Rust` | 74.7 ± 6.4 | 53.8 | 84.2 | 1.00 |
| `Go` | 665.7 ± 20.8 | 609.9 | 692.5 | 8.91 ± 0.81 |
| `Scala` | 380.4 ± 9.8 | 356.6 | 397.8 | 5.09 ± 0.45 |
| `C++` | 206.7 ± 6.0 | 184.6 | 214.9 | 2.77 ± 0.25 |

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
| `Rust` | 2.7 ± 0.3 | 1.6 | 3.4 | 1.00 |
| `Go` | 3.9 ± 0.7 | 2.4 | 5.1 | 1.48 ± 0.31 |
| `Scala` | 354.0 ± 8.2 | 341.5 | 367.9 | 133.42 ± 15.65 |
| `C++` | 1070.3 ± 8.8 | 1047.1 | 1087.9 | 403.37 ± 46.49 |

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
| `Rust` | 16.1 ± 1.8 | 9.5 | 19.2 | 1.00 |
| `Go` | 18.9 ± 2.4 | 11.1 | 23.3 | 1.18 ± 0.20 |
| `Scala` | 357.1 ± 11.4 | 327.2 | 378.3 | 22.24 ± 2.54 |
| `C++` | 38984.2 ± 160.9 | 38481.9 | 39276.8 | 2428.07 ± 266.80 |

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
| `Rust` | 3.5 ± 0.5 | 2.1 | 4.4 | 1.00 |
| `Go` | 6.9 ± 1.2 | 4.0 | 8.8 | 2.00 ± 0.43 |
| `Scala` | 342.5 ± 11.1 | 315.1 | 365.0 | 99.05 ± 13.39 |
| `C++` | 42.1 ± 11.4 | 28.4 | 55.3 | 12.18 ± 3.67 |

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
| `Rust` | 4.2 ± 0.6 | 2.6 | 5.9 | 1.00 |
| `Go` | 27.1 ± 3.2 | 16.4 | 32.6 | 6.44 ± 1.17 |
| `Scala` | 377.4 ± 9.2 | 360.2 | 394.0 | 89.59 ± 12.70 |
| `C++` | 15045.5 ± 112.9 | 14763.1 | 15288.9 | 3571.12 ± 499.61 |

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
| `Rust` | 4.1 ± 0.6 | 2.5 | 5.5 | 1.00 |
| `Go` | 25.1 ± 2.9 | 16.1 | 29.4 | 6.18 ± 1.10 |
| `Scala` | 377.5 ± 10.4 | 358.5 | 397.1 | 93.09 ± 13.03 |
| `C++` | 10304.6 ± 62.8 | 10154.3 | 10389.1 | 2540.95 ± 349.00 |

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
| `Rust` | 54.8 ± 10.4 | 34.7 | 68.3 | 1.00 |
| `Go` | 422.7 ± 17.7 | 387.7 | 447.7 | 7.72 ± 1.50 |
| `Scala` | 425.6 ± 12.4 | 392.0 | 444.2 | 7.77 ± 1.49 |
| `C++` | 10028.3 ± 42.1 | 9937.1 | 10106.0 | 183.13 ± 34.75 |

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
| `Rust` | 49.8 ± 10.1 | 30.5 | 58.2 | 1.00 |
| `Go` | 268.9 ± 5.2 | 250.5 | 274.8 | 5.40 ± 1.10 |
| `Scala` | 427.9 ± 10.0 | 413.1 | 448.0 | 8.59 ± 1.75 |
| `C++` | 13958.9 ± 151.1 | 13822.2 | 14478.0 | 280.37 ± 56.84 |

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
| `Rust` | 9.1 ± 1.4 | 5.2 | 10.7 | 1.00 |
| `Go` | 44.6 ± 4.0 | 36.0 | 51.2 | 4.92 ± 0.86 |
| `Scala` | 463.9 ± 13.9 | 428.4 | 486.6 | 51.22 ± 7.86 |
| `C++` | 274.5 ± 9.3 | 255.0 | 286.9 | 30.31 ± 4.68 |

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
| `Rust` | 2.7 ± 0.3 | 1.6 | 3.4 | 1.00 |
| `Go` | 5.2 ± 0.9 | 2.9 | 6.5 | 1.93 ± 0.40 |
| `Scala` | 350.7 ± 12.8 | 315.6 | 370.8 | 130.60 ± 15.89 |
| `C++` | 124.6 ± 8.5 | 105.9 | 133.3 | 46.40 ± 6.25 |

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
| `Rust` | 2.8 ± 0.3 | 1.7 | 3.7 | 1.00 |
| `Go` | 5.4 ± 0.8 | 2.9 | 6.9 | 1.94 ± 0.37 |
| `C++` | 1118.3 ± 21.4 | 1094.0 | 1190.0 | 405.19 ± 44.55 |

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
| `Rust` | 2.8 ± 0.3 | 1.7 | 3.7 | 1.00 |
| `Go` | 4.8 ± 0.7 | 2.7 | 6.2 | 1.72 ± 0.33 |
| `Scala` | 357.2 ± 11.3 | 334.5 | 377.2 | 128.46 ± 14.87 |
| `C++` | 126.7 ± 6.8 | 105.8 | 134.7 | 45.57 ± 5.63 |

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
| `Rust` | 63.1 ± 11.3 | 46.0 | 77.6 | 1.00 |
| `Go` | 487.1 ± 31.1 | 441.0 | 553.5 | 7.72 ± 1.47 |
| `C++` | 712.8 ± 11.2 | 693.0 | 742.5 | 11.30 ± 2.03 |

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
| `Rust` | 3.6 ± 0.3 | 2.7 | 4.5 | 1.00 |
| `Go` | 11.0 ± 1.6 | 6.9 | 13.5 | 3.09 ± 0.52 |
| `Scala` | 359.7 ± 9.8 | 345.8 | 374.6 | 100.48 ± 8.47 |
| `C++` | 49.4 ± 10.0 | 29.3 | 62.7 | 13.80 ± 3.00 |

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
| `Rust` | 45.0 ± 10.3 | 26.6 | 55.2 | 1.00 |
| `Go` | 160.8 ± 17.0 | 116.2 | 182.1 | 3.57 ± 0.90 |
| `C++` | 135271.6 ± 10230.4 | 130120.3 | 172035.4 | 3002.97 ± 720.97 |

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
| `Rust` | 414.7 ± 10.1 | 394.7 | 437.9 | 1.00 |
| `Go` | 2087.0 ± 36.5 | 2030.6 | 2148.9 | 5.03 ± 0.15 |
| `Scala` | 535.1 ± 9.9 | 514.5 | 556.0 | 1.29 ± 0.04 |
