
# EvaluationJob

An evaluation match, fully resolved: the candidate plays pairs of games against the baseline from the same random opening with colors swapped.

## Properties

Name | Type
------------ | -------------
`baseline` | [Contestant](Contestant.md)
`candidate` | [Contestant](Contestant.md)
`games` | number
`moveTimeMs` | number
`nodes` | number
`seed` | number
`sprt` | [Sprt](Sprt.md)
`threads` | number

## Example

```typescript
import type { EvaluationJob } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "baseline": null,
  "candidate": null,
  "games": null,
  "moveTimeMs": null,
  "nodes": null,
  "seed": null,
  "sprt": null,
  "threads": null,
} satisfies EvaluationJob

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as EvaluationJob
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


