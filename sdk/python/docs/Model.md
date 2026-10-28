# Model

A trained network.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**architecture** | [**Architecture**](Architecture.md) |  | 
**architecture_hash** | **str** | Its hash. | 
**bytes** | **int** | Size of the network file. | 
**created_at** | **int** | Creation time (Unix milliseconds). | 
**file_sha256** | **str** | SHA-256 of the network file. | 
**id** | **UUID** | Identifier. | 
**job** | **UUID** | Training job that produced it. | [optional] 
**name** | **str** | Name (the training job&#39;s). | 
**provenance** | [**Provenance**](Provenance.md) | How it was made. | 

## Example

```python
from chess_nnue_lab.models.model import Model

# TODO update the JSON string below
json = "{}"
# create an instance of Model from a JSON string
model_instance = Model.from_json(json)
# print the JSON string representation of the object
print(Model.to_json())

# convert the object into a dict
model_dict = model_instance.to_dict()
# create an instance of Model from a dict
model_from_dict = Model.from_dict(model_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


