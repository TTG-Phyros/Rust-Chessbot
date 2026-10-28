
# Estimate

Size and cost of an architecture, to compare designs before training.

## Properties

Name | Type
------------ | -------------
`accumulatorBytes` | number
`costVsLegacy` | number
`denseMacs` | number
`inputFeatures` | number
`kingRefreshes` | boolean
`parameters` | number
`refreshOps` | number
`updateOps` | number
`weightsBytes` | number

## Example

```typescript
import type { Estimate } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "accumulatorBytes": null,
  "costVsLegacy": null,
  "denseMacs": null,
  "inputFeatures": null,
  "kingRefreshes": null,
  "parameters": null,
  "refreshOps": null,
  "updateOps": null,
  "weightsBytes": null,
} satisfies Estimate

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Estimate
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


