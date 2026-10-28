
# Features

Input features.  Each perspective sees the board from its own side: squares are flipped vertically for Black, and pieces are \"own\" or \"opponent\". With king buckets, the features are repeated once per bucket and the bucket of the perspective\'s own king selects which copy is active.

## Properties

Name | Type
------------ | -------------
`kingBuckets` | Array&lt;number&gt;
`mirror` | [Mirror](Mirror.md)
`perspective` | boolean
`set` | [FeatureSet](FeatureSet.md)

## Example

```typescript
import type { Features } from 'chess-nnue-lab'

// TODO: Update the object below with actual values
const example = {
  "kingBuckets": null,
  "mirror": null,
  "perspective": null,
  "set": null,
} satisfies Features

console.log(example)

// Convert the instance to a JSON string
const exampleJSON: string = JSON.stringify(example)
console.log(exampleJSON)

// Parse the JSON string back to an object
const exampleParsed = JSON.parse(exampleJSON) as Features
console.log(exampleParsed)
```

[[Back to top]](#) [[Back to API list]](../README.md#api-endpoints) [[Back to Model list]](../README.md#models) [[Back to README]](../README.md)


