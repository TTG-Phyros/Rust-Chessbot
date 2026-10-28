
# SelfplayRequest

A self-play dataset, as a client asks for it.

## Properties

Name | Type
------------ | -------------
`games` | number
`name` | string
`network` | string
`nodes` | number
`seed` | number
`threads` | number

## Example

```typescript
import type { SelfplayRequest } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "games": null,
  "name": null,
  "network": null,
  "nodes": null,
  "seed": null,
  "threads": null,
} satisfies SelfplayRequest

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as SelfplayRequest
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


