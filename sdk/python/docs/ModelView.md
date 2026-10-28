# ModelView

A model, and whether the engine plays with it.

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
**active** | **bool** | Whether it is the active model. | 

## Example

```python
from chess_nnue_lab.models.model_view import ModelView

# TODO update the JSON string below
json = "{}"
# create an instance of ModelView from a JSON string
model_view_instance = ModelView.from_json(json)
# print the JSON string representation of the object
print(ModelView.to_json())

# convert the object into a dict
model_view_dict = model_view_instance.to_dict()
# create an instance of ModelView from a dict
model_view_from_dict = ModelView.from_dict(model_view_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


