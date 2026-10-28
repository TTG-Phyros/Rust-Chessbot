
# Sprt

A sequential probability ratio test: is the first player at least `elo1` stronger (H1), or at most `elo0` (H0)?

## Properties

Name | Type
------------ | -------------
`alpha` | number
`beta` | number
`elo0` | number
`elo1` | number

## Example

```typescript
import type { Sprt } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "alpha": null,
  "beta": null,
  "elo0": null,
  "elo1": null,
} satisfies Sprt

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Sprt
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


