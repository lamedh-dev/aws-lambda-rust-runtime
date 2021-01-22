# 0.3.0

## Features

- Export basic Error type
- Crates rename to be company agnostic

# 0.2.0

## Features

- Tokio 1.0 support (https://github.com/netlify/aws-lambda-rust-runtime/pull/8)

## Bug Fixes

- HTTP deserialization fixed for Invoke requests (https://github.com/netlify/aws-lambda-rust-runtime/pull/12)

# 0.1.1

- Fix types to work with the AWS Runtime Emulator

# 0.1.0

- Initial Fork
- Publish new crates with `netlify_` prefix: `netlify_lambda`, `netlify_lambda_http`, `netlify_lambda_attributes`
- Provide enum types for different HTTP requests
- Fix logging
- Fix types to work with the Invoke API
