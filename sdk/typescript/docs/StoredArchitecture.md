
# StoredArchitecture

An architecture saved for reuse.

## Properties

Name | Type
------------ | -------------
`architecture` | [Architecture](Architecture.md)
`createdAt` | number
`hash` | string
`id` | string
`name` | string

## Example

```typescript
import type { StoredArchitecture } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "architecture": null,
  "createdAt": null,
  "hash": null,
  "id": null,
  "name": null,
} satisfies StoredArchitecture

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as StoredArchitecture
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


