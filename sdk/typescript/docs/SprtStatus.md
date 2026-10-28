
# SprtStatus

Where a SPRT stands.

## Properties

Name | Type
------------ | -------------
`llr` | number
`lowerBound` | number
`upperBound` | number
`verdict` | [Verdict](Verdict.md)

## Example

```typescript
import type { SprtStatus } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "llr": null,
  "lowerBound": null,
  "upperBound": null,
  "verdict": null,
} satisfies SprtStatus

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as SprtStatus
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


