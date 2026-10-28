# OutputBuckets

Output buckets.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**count** | **int** | Number of buckets: required unless the strategy is &#x60;none&#x60;. With &#x60;pieceCount&#x60;, a divisor of 32. | [optional] 
**scope** | [**BucketScope**](BucketScope.md) | Bucketed layers: the output layer only (default) or every layer after the accumulator. | [optional] 
**strategy** | [**BucketStrategy**](BucketStrategy.md) | How a position picks its bucket. | [optional] 

## Example

```python
from chess_nnue_lab.models.output_buckets import OutputBuckets

# TODO update the JSON string below
json = "{}"
# create an instance of OutputBuckets from a JSON string
output_buckets_instance = OutputBuckets.from_json(json)
# print the JSON string representation of the object
print(OutputBuckets.to_json())

# convert the object into a dict
output_buckets_dict = output_buckets_instance.to_dict()
# create an instance of OutputBuckets from a dict
output_buckets_from_dict = OutputBuckets.from_dict(output_buckets_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


