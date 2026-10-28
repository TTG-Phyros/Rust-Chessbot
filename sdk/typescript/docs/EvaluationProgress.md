
# EvaluationProgress

Latest result of an evaluation match, from the candidate\'s side.

## Properties

Name | Type
------------ | -------------
`baselineNps` | number
`candidateNps` | number
`draws` | number
`elo` | number
`eloError` | number
`games` | number
`gamesMax` | number
`los` | number
`losses` | number
`score` | number
`sprt` | [SprtStatus](SprtStatus.md)
`wins` | number

## Example

```typescript
import type { EvaluationProgress } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "baselineNps": null,
  "candidateNps": null,
  "draws": null,
  "elo": null,
  "eloError": null,
  "games": null,
  "gamesMax": null,
  "los": null,
  "losses": null,
  "score": null,
  "sprt": null,
  "wins": null,
} satisfies EvaluationProgress

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as EvaluationProgress
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


