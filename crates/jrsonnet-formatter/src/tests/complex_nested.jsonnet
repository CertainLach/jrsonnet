
			{
				kubernetes: {
				 deployment: {
					apiVersion: 'apps/v1',
					kind: 'Deployment',
					metadata: {
					  name: 'myapp',
					  labels: { app: 'myapp', version: 'v1' },
					},
					spec: {
					  replicas: 3,
					  selector: { matchLabels: { app: 'myapp' } },
					  template: {
						 metadata: { labels: { app: 'myapp' } },
						 spec: {
							containers: [
							  {
								 name: 'myapp',
								 image: 'myapp:latest',
								 ports: [{ containerPort: 8080 }],
								 env: [
									{ name: 'FOO', value: 'bar' },
									{ name: 'BAZ', valueFrom: { secretKeyRef: { name: 'mysecret', key: 'password' } } },
								 ],
							  },
							],
						 },
					  },
					},
				 },
			  },
			}
