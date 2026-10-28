# Features

Input features.  Each perspective sees the board from its own side: squares are flipped vertically for Black, and pieces are \"own\" or \"opponent\". With king buckets, the features are repeated once per bucket and the bucket of the perspective's own king selects which copy is active.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**king_buckets** | **List[int]** | Bucket of each square of the perspective&#39;s own king, 64 entries from a1 (index 0, own back rank) to h8 (index 63). Ids are 0..N-1, all used. Default: one bucket for &#x60;Chess768&#x60;, one per square for &#x60;HalfKP&#x60;, one per square of files a–d for &#x60;HalfKAv2_hm&#x60;. | [optional] 
**mirror** | [**Mirror**](Mirror.md) | Horizontal mirroring: with &#x60;kingFile&#x60;, a perspective whose own king stands on files e–h sees the board mirrored (a↔h), so its king is always on files a–d. Default: &#x60;kingFile&#x60; for &#x60;HalfKAv2_hm&#x60;, &#x60;none&#x60; otherwise. | [optional] 
**perspective** | **bool** | Two accumulators, one per side, the side to move&#39;s first in the next layer&#39;s input. Without it, a single accumulator sees the board as White and the result is negated when Black is to move. | [optional] [default to True]
**set** | [**FeatureSet**](FeatureSet.md) | Piece encoding: &#x60;Chess768&#x60;: 12 planes (own/opponent × 6 kinds, kings included) × 64 squares; &#x60;HalfKP&#x60;: 10 planes (kings excluded) × 64, one copy per king square by default; &#x60;HalfKAv2_hm&#x60;: 11 planes (both kings share one) × 64, mirrored, 32 king buckets by default. | 

## Example

```python
from chess_nnue_lab.models.features import Features

# TODO update the JSON string below
json = "{}"
# create an instance of Features from a JSON string
features_instance = Features.from_json(json)
# print the JSON string representation of the object
print(Features.to_json())

# convert the object into a dict
features_dict = features_instance.to_dict()
# create an instance of Features from a dict
features_from_dict = Features.from_dict(features_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


