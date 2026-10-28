
# Progress

Latest progress of a job.

## Properties

Name | Type
------------ | -------------
`learningRate` | number
`loss` | number
`positions` | number
`positionsPerSecond` | number
`step` | number
`superbatch` | number
`superbatches` | number
`vramUsedMb` | number
`kind` | string
`games` | number
`gamesTotal` | number
`baselineNps` | number
`candidateNps` | number
`draws` | number
`elo` | number
`eloError` | number
`gamesMax` | number
`los` | number
`losses` | number
`score` | number
`sprt` | [SprtStatus](SprtStatus.md)
`wins` | number

## Example

```typescript
import type { Progress } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "learningRate": null,
  "loss": null,
  "positions": null,
  "positionsPerSecond": null,
  "step": null,
  "superbatch": null,
  "superbatches": null,
  "vramUsedMb": null,
  "kind": null,
  "games": null,
  "gamesTotal": null,
  "baselineNps": null,
  "candidateNps": null,
  "draws": null,
  "elo": null,
  "eloError": null,
  "gamesMax": null,
  "los": null,
  "losses": null,
  "score": null,
  "sprt": null,
  "wins": null,
} satisfies Progress

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Progress
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


