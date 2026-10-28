
# ProgressOneOf1

See [`SelfplayProgress`].

## Properties

Name | Type
------------ | -------------
`games` | number
`gamesTotal` | number
`positions` | number
`kind` | string

## Example

```typescript
import type { ProgressOneOf1 } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "games": null,
  "gamesTotal": null,
  "positions": null,
  "kind": null,
} satisfies ProgressOneOf1

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as ProgressOneOf1
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


