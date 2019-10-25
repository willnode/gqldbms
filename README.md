# GraphQL-Powered Database Management Server

A drop-in server for database with GraphQL. No need to write resolvers, No need to dangle with SQL transactions. Just write the schema and run!

**Warning**: Currently it's very experimental, minimalistic, and insecure.

## How it Works

1. This server uses JSON to save data
2. GraphQL objects are stored as JSON objects wrapped in array
3. `TODO`

## Running

1. Clone
2. `cargo run`
3. Go to `localhost:3000/graphiql`