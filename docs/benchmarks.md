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
Expected Signature: Sjsonnet 0.4.10
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

Rust (alternative):

```
Usage: rsjsonnet [OPTIONS] <filename>

Arguments:
  <filename>  

Options:
  -e, --exec                      Treat filename as code
  -J, --jpath <dir>               Specify an additional library search dir (right-most wins)
  -o, --output-file <file>        Write to the output file rather than stdout
  -m, --multi <dir>               Write multiple files to the directory, list files on stdout
  -y, --yaml-stream               Write output as a YAML stream of JSON documents
  -S, --string                    Expect a string, manifest as plain text
  -s, --max-stack <n>             Number of allowed stack frames
  -t, --max-trace <n>             Max length of stack trace before cropping
  -V, --ext-str <var=[val]>       
      --ext-str-file <var=file>   
      --ext-code <var[=code]>     
      --ext-code-file <var=file>  
  -A, --tla-str <var[=val]>       
      --tla-str-file <var=file>   
      --tla-code <var[=code]>     
      --tla-code-file <var=file>  
  -h, --help                      Print help

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
| `Rust` | 93.6 ± 0.6 | 92.5 | 94.7 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 263.3 ± 1.8 | 259.0 | 268.4 | 2.81 ± 0.03 |
| `Go` | 1204.6 ± 5.0 | 1196.0 | 1215.4 | 12.87 ± 0.10 |
| `Scala` | 720.0 ± 2.9 | 713.4 | 725.3 | 7.70 ± 0.06 |

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
| `Rust` | 129.4 ± 1.4 | 127.7 | 132.8 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 197.0 ± 1.9 | 193.6 | 200.7 | 1.52 ± 0.02 |
| `Go` | 2127.7 ± 13.8 | 2105.4 | 2151.5 | 16.44 ± 0.21 |
| `Scala` | 947.9 ± 11.1 | 926.1 | 967.9 | 7.33 ± 0.12 |
| `C++` | 87633.8 ± 569.9 | 86464.2 | 89118.8 | 677.29 ± 8.72 |

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
| `Rust` | 5.6 ± 0.1 | 5.4 | 6.2 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 45.3 ± 0.9 | 43.5 | 47.8 | 8.04 ± 0.25 |
| `Go` | 98.2 ± 1.6 | 95.0 | 100.8 | 17.45 ± 0.50 |
| `Scala` | 331.1 ± 5.8 | 322.3 | 343.7 | 58.79 ± 1.73 |
| `C++` | 76.5 ± 0.7 | 75.4 | 78.0 | 13.59 ± 0.34 |

### Large string template

> Note: No results for Go, fails with os stack size exhausion

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 6.7 ± 0.1 | 6.4 | 7.1 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 7.1 ± 0.2 | 6.8 | 7.6 | 1.05 ± 0.03 |
| `Scala` | 392.7 ± 2.7 | 388.2 | 399.8 | 58.29 ± 1.19 |
| `C++` | 14376.2 ± 35.8 | 14317.5 | 14448.6 | 2133.86 ± 41.53 |

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
| `Rust` | 12.6 ± 0.1 | 12.3 | 12.9 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 49.7 ± 1.0 | 47.3 | 53.0 | 3.96 ± 0.09 |
| `Go` | 6858.1 ± 34.9 | 6815.9 | 6951.8 | 546.32 ± 6.87 |
| `Scala` | 382.3 ± 3.9 | 371.7 | 387.1 | 30.46 ± 0.47 |
| `C++` | 24472.7 ± 155.7 | 24230.3 | 24799.3 | 1949.51 ± 25.63 |

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
| `Rust` | 259.9 ± 4.4 | 250.9 | 266.7 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 550.2 ± 16.6 | 531.9 | 600.7 | 2.12 ± 0.07 |
| `Go` | 6732.4 ± 51.4 | 6660.2 | 6858.6 | 25.90 ± 0.48 |
| `Scala` | 675.4 ± 10.9 | 667.3 | 720.0 | 2.60 ± 0.06 |
| `C++` | 26414.5 ± 259.9 | 25948.9 | 26934.4 | 101.63 ± 1.99 |

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
| `Rust` | 1.8 ± 0.1 | 1.6 | 2.6 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 3.1 ± 0.1 | 2.8 | 3.4 | 1.69 ± 0.12 |
| `Go` | 5.3 ± 0.2 | 4.9 | 5.9 | 2.90 ± 0.20 |
| `Scala` | 287.4 ± 1.2 | 285.4 | 288.9 | 156.55 ± 9.92 |
| `C++` | 32.1 ± 0.3 | 31.6 | 33.0 | 17.49 ± 1.12 |

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
| `Rust` | 376.1 ± 5.2 | 364.3 | 383.7 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 496.7 ± 13.0 | 470.6 | 506.8 | 1.32 ± 0.04 |
| `Go` | 873.9 ± 7.1 | 859.3 | 888.0 | 2.32 ± 0.04 |
| `Scala` | 420.8 ± 6.3 | 413.9 | 437.1 | 1.12 ± 0.02 |
| `C++` | 2647.5 ± 14.2 | 2630.9 | 2675.6 | 7.04 ± 0.10 |

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
| `Rust` | 81.3 ± 0.3 | 80.9 | 82.4 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 202.3 ± 12.0 | 183.1 | 237.6 | 2.49 ± 0.15 |
| `Go` | 349.4 ± 1.7 | 347.2 | 354.2 | 4.30 ± 0.03 |
| `Scala` | 322.5 ± 3.0 | 318.7 | 330.7 | 3.97 ± 0.04 |
| `C++` | 193.5 ± 0.9 | 191.4 | 194.8 | 2.38 ± 0.01 |

### Foldl string concat

<details>
<summary>Source</summary>

```jsonnet
std.foldl(function(e, res) e + res, std.makeArray(20000, function(i) 'aaaaa'), '')

```
</details>

| Command | Mean [ms] | Min [ms] | Max [ms] | Relative |
|:---|---:|---:|---:|---:|
| `Rust` | 8.9 ± 0.1 | 8.6 | 9.3 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 303.1 ± 3.8 | 297.6 | 308.1 | 34.06 ± 0.68 |
| `Go` | 50.9 ± 0.8 | 48.3 | 52.2 | 5.72 ± 0.13 |
| `Scala` | 550.6 ± 5.5 | 542.8 | 563.7 | 61.86 ± 1.13 |
| `C++` | 902.6 ± 4.8 | 891.4 | 912.1 | 101.41 ± 1.65 |

### Array sorts

> Note: No results for Scala, sjsonnet doesn't support keyF in std.sort: https://github.com/databricks/sjsonnet/issues/204

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
| `Rust` | 3.2 ± 0.2 | 2.9 | 4.1 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 14.4 ± 0.6 | 13.1 | 16.2 | 4.46 ± 0.28 |
| `Go` | 12.1 ± 0.2 | 11.4 | 12.8 | 3.74 ± 0.20 |
| `C++` | 3637.1 ± 24.9 | 3557.4 | 3664.7 | 1128.64 ± 56.81 |

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
| `Rust` | 54.1 ± 1.1 | 52.3 | 56.3 | 1.30 ± 0.03 |
| `Rust (alternative, rsjsonnet)` | 41.7 ± 0.7 | 39.6 | 43.8 | 1.00 |
| `Go` | 596.5 ± 5.2 | 585.1 | 606.9 | 14.30 ± 0.29 |
| `Scala` | 306.6 ± 1.6 | 303.5 | 310.4 | 7.35 ± 0.14 |
| `C++` | 184.5 ± 1.8 | 182.0 | 187.9 | 4.42 ± 0.09 |

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
| `Rust` | 1.6 ± 0.1 | 1.5 | 2.5 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 2.9 ± 0.1 | 2.7 | 3.7 | 1.78 ± 0.13 |
| `Go` | 2.4 ± 0.1 | 2.1 | 2.9 | 1.48 ± 0.12 |
| `Scala` | 293.2 ± 1.5 | 289.9 | 296.1 | 178.20 ± 12.35 |
| `C++` | 999.1 ± 9.2 | 974.4 | 1010.9 | 607.23 ± 42.34 |

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
| `Rust` | 8.6 ± 0.2 | 8.4 | 9.5 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 18.4 ± 0.4 | 17.6 | 20.8 | 2.13 ± 0.06 |
| `Go` | 11.2 ± 0.2 | 10.4 | 11.9 | 1.29 ± 0.04 |
| `Scala` | 294.9 ± 2.0 | 292.2 | 301.9 | 34.10 ± 0.65 |
| `C++` | 37345.3 ± 208.2 | 36567.2 | 37689.0 | 4318.10 ± 80.13 |

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
| `Rust` | 2.2 ± 0.1 | 1.9 | 3.0 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 3.2 ± 0.1 | 2.9 | 3.5 | 1.46 ± 0.10 |
| `Go` | 4.1 ± 0.1 | 3.7 | 4.6 | 1.85 ± 0.13 |
| `Scala` | 292.3 ± 2.9 | 289.3 | 304.0 | 132.11 ± 8.64 |
| `C++` | 28.3 ± 0.3 | 27.7 | 29.1 | 12.80 ± 0.84 |

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
| `Rust` | 2.7 ± 0.1 | 2.4 | 3.4 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 2195.9 ± 48.5 | 2098.1 | 2229.6 | 804.07 ± 39.58 |
| `Go` | 17.0 ± 0.3 | 16.1 | 17.7 | 6.22 ± 0.29 |
| `Scala` | 313.8 ± 1.2 | 311.3 | 316.1 | 114.91 ± 5.07 |
| `C++` | 14621.6 ± 110.0 | 14473.9 | 14774.9 | 5354.05 ± 238.94 |

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
| `Rust` | 2.6 ± 0.1 | 2.3 | 3.1 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 7889.2 ± 74.7 | 7711.6 | 8051.1 | 3045.22 ± 138.95 |
| `Go` | 15.2 ± 0.3 | 14.6 | 16.5 | 5.85 ± 0.28 |
| `Scala` | 313.0 ± 2.5 | 309.5 | 321.2 | 120.80 ± 5.47 |
| `C++` | 9930.9 ± 26.5 | 9895.0 | 9997.9 | 3833.33 ± 171.41 |

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
| `Rust` | 34.8 ± 0.5 | 33.4 | 36.0 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 8116.2 ± 37.2 | 8049.1 | 8176.2 | 233.52 ± 3.24 |
| `Go` | 271.1 ± 2.3 | 267.1 | 275.7 | 7.80 ± 0.12 |
| `Scala` | 357.2 ± 2.3 | 353.5 | 363.4 | 10.28 ± 0.15 |
| `C++` | 9653.7 ± 22.5 | 9621.4 | 9694.3 | 277.75 ± 3.69 |

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
| `Rust` | 30.3 ± 0.4 | 29.1 | 31.6 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 1460.8 ± 47.6 | 1375.2 | 1568.3 | 48.26 ± 1.72 |
| `Go` | 156.5 ± 1.9 | 152.5 | 159.4 | 5.17 ± 0.10 |
| `Scala` | 355.9 ± 2.9 | 351.4 | 365.6 | 11.76 ± 0.20 |
| `C++` | 13362.4 ± 88.3 | 13028.4 | 13473.8 | 441.41 ± 7.01 |

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
| `Rust` | 5.3 ± 0.2 | 5.0 | 6.2 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 81.5 ± 1.6 | 78.9 | 83.8 | 15.38 ± 0.55 |
| `Go` | 26.6 ± 0.5 | 25.0 | 27.5 | 5.02 ± 0.18 |
| `Scala` | 405.4 ± 3.0 | 400.9 | 413.6 | 76.52 ± 2.34 |
| `C++` | 265.3 ± 2.7 | 260.6 | 271.5 | 50.08 ± 1.57 |

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
| `Rust` | 1.7 ± 0.1 | 1.4 | 2.7 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 2.9 ± 0.1 | 2.7 | 3.3 | 1.73 ± 0.13 |
| `Go` | 3.2 ± 0.1 | 2.7 | 3.8 | 1.92 ± 0.15 |
| `Scala` | 294.7 ± 1.7 | 291.6 | 299.0 | 175.84 ± 12.30 |
| `C++` | 101.4 ± 0.8 | 99.3 | 102.9 | 60.48 ± 4.24 |

### std.manifestTomlEx

> Note: No results for Scala, std.manifestTomlEx is not implemented: https://github.com/databricks/sjsonnet/issues/111

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
| `Rust` | 1.7 ± 0.1 | 1.4 | 2.7 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 7.8 ± 0.2 | 7.3 | 8.5 | 4.65 ± 0.36 |
| `Go` | 3.2 ± 0.1 | 2.9 | 3.7 | 1.90 ± 0.16 |
| `C++` | 1046.1 ± 4.9 | 1039.7 | 1053.9 | 622.36 ± 44.25 |

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
| `Rust` | 1.7 ± 0.1 | 1.4 | 2.6 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 2.9 ± 0.1 | 2.6 | 3.4 | 1.71 ± 0.16 |
| `Go` | 2.8 ± 0.1 | 2.4 | 3.3 | 1.64 ± 0.16 |
| `Scala` | 292.7 ± 1.7 | 289.2 | 295.8 | 170.68 ± 14.76 |
| `C++` | 101.0 ± 0.7 | 99.0 | 102.0 | 58.88 ± 5.10 |

### std.reverse

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
| `Rust` | 45.3 ± 0.5 | 44.0 | 46.5 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 784.1 ± 20.4 | 741.5 | 802.0 | 17.31 ± 0.49 |
| `Go` | 327.4 ± 2.1 | 323.8 | 331.4 | 7.23 ± 0.10 |
| `Scala` | 367.0 ± 2.4 | 361.1 | 370.2 | 8.10 ± 0.11 |
| `C++` | 659.3 ± 5.4 | 644.6 | 666.0 | 14.55 ± 0.21 |

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
| `Rust` | 2.2 ± 0.2 | 1.9 | 3.2 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 3.2 ± 0.1 | 3.1 | 3.6 | 1.49 ± 0.11 |
| `Go` | 6.8 ± 0.2 | 6.3 | 7.4 | 3.12 ± 0.24 |
| `Scala` | 303.6 ± 3.6 | 300.5 | 318.2 | 139.43 ± 9.76 |
| `C++` | 29.1 ± 0.4 | 28.3 | 30.2 | 13.36 ± 0.94 |

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
| `Rust` | 25.7 ± 0.2 | 25.0 | 26.0 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 228.1 ± 7.9 | 214.0 | 245.1 | 8.89 ± 0.31 |
| `Go` | 150.4 ± 10.4 | 135.9 | 169.4 | 5.86 ± 0.41 |
| `C++` | 125755.8 ± 989.6 | 123095.8 | 127475.4 | 4901.85 ± 49.12 |

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
| `Rust` | 385.9 ± 4.2 | 372.4 | 391.5 | 1.00 |
| `Rust (alternative, rsjsonnet)` | 1287.4 ± 27.0 | 1244.2 | 1335.1 | 3.34 ± 0.08 |
| `Go` | 1817.1 ± 18.6 | 1779.7 | 1842.9 | 4.71 ± 0.07 |
| `Scala` | 453.1 ± 3.9 | 448.1 | 462.2 | 1.17 ± 0.02 |
