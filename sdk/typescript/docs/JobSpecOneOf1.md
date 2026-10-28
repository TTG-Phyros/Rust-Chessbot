
# JobSpecOneOf1

See [`SelfplayJob`].

## Properties

Name | Type
------------ | -------------
`dataset` | string
`games` | number
`network` | string
`nodes` | number
`seed` | number
`threads` | number
`kind` | string

## Example

```typescript
import type { JobSpecOneOf1 } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "dataset": null,
  "games": null,
  "network": null,
  "nodes": null,
  "seed": null,
  "threads": null,
  "kind": null,
} satisfies JobSpecOneOf1

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as JobSpecOneOf1
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


