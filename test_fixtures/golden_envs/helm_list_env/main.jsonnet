// Test case: helmTemplate with List kind resources
// This tests that resources wrapped in a Kubernetes List kind are properly expanded
// Similar to how aws-load-balancer-controller wraps IngressClass and IngressClassParams

local helmResources = std.native('helmTemplate')(
  'list-test',
  './charts/list-chart',
  {
    calledFrom: std.thisFile,
    namespace: 'kube-system',
    values: {
      createListResource: true,
      ingressClass: 'alb',
    },
  }
);

// Inline environment with helm resources
{
  apiVersion: 'tanka.dev/v1alpha1',
  kind: 'Environment',
  metadata: {
    name: 'helm-list-test',
    labels: {
      cluster: 'test-cluster',
    },
  },
  spec: {
    apiServer: 'https://fwnkiegyk:6443',
    namespace: 'kube-system',
  },
  data: helmResources,
}
