# Dataset

A dataset.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**bytes** | **int** | Size of its files. | 
**created_at** | **int** | Creation time (Unix milliseconds). | 
**error** | **str** | Why it failed. | [optional] 
**id** | **UUID** | Identifier. | 
**job** | **UUID** | Self-play job generating it. | [optional] 
**name** | **str** | Name given at creation. | 
**records** | **int** | Positions. | 
**sha256** | **str** | Content hash (SHA-256 of the records), once ready. | [optional] 
**source** | [**DatasetSource**](DatasetSource.md) | Origin. | 
**stats** | [**DatasetStats**](DatasetStats.md) | Statistics, once ready. | [optional] 
**status** | [**DatasetStatus**](DatasetStatus.md) | Status. | 
**updated_at** | **int** | Last change. | 

## Example

```python
from chess_nnue_lab.models.dataset import Dataset

# TODO update the JSON string below
json = "{}"
# create an instance of Dataset from a JSON string
dataset_instance = Dataset.from_json(json)
# print the JSON string representation of the object
print(Dataset.to_json())

# convert the object into a dict
dataset_dict = dataset_instance.to_dict()
# create an instance of Dataset from a dict
dataset_from_dict = Dataset.from_dict(dataset_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


