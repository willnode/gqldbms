# GraphQL-Powered Database Management Server

A simple drop-in web server written in Rust for database powered with [GraphQL](https://graphql.org).

No need to write resolvers, No need to depends on separate SQL servers. Just write the schema and run!

**Warning**: Currently it's very experimental, unoptimized, insecure, and only works with simple queries.

## How it Works

1. This server uses [JSON to save data](public/data.json)
2. GraphQL objects are stored as JSON objects wrapped in array
3. Listening to schema that [defined by yours](public/schema.gql)

## Running

1. Clone
2. `cargo run`
3. Go to `localhost:3000/graphiql`
4. Test it:

```graphql
query AuthorWithPosts {
  author(id: 1) {
    username
    posts {
      id
      title
    }
  }
  feed {
    id
    title
    author {
      username
    }
  }
}
```

returns:

```json
{
  "data": {
    "author": [
      {
        "posts": [
          {
            "id": 1,
            "title": "Skywalker off the Road"
          },
          {
            "id": 2,
            "title": "Truth of Science"
          }
        ],
        "username": "John"
      }
    ],
    "feed": [
      {
        "author": {
          "username": "John"
        },
        "id": 1,
        "title": "Skywalker off the Road"
      },
      {
        "author": {
          "username": "John"
        },
        "id": 2,
        "title": "Truth of Science"
      },
      {
        "author": {
          "username": "Alex"
        },
        "id": 3,
        "title": "Celebrating Alex"
      }
    ]
  }
}
```