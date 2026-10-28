
# EvaluationRequest

An evaluation match, as a client asks for it.

## Properties

Name | Type
------------ | -------------
`baseline` | string
`candidate` | string
`games` | number
`moveTimeMs` | number
`name` | string
`nodes` | number
`seed` | number
`sprt` | [Sprt](Sprt.md)
`threads` | number

## Example

```typescript
import type { EvaluationRequest } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "baseline": null,
  "candidate": null,
  "games": null,
  "moveTimeMs": null,
  "name": null,
  "nodes": null,
  "seed": null,
  "sprt": null,
  "threads": null,
} satisfies EvaluationRequest

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as EvaluationRequest
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


