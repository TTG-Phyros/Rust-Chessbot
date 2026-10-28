# StoredArchitecture

An architecture saved for reuse.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**architecture** | [**Architecture**](Architecture.md) |  | 
**created_at** | **int** | Creation time (Unix milliseconds). | 
**hash** | **str** | Hash. | 
**id** | **UUID** | Identifier. | 
**name** | **str** | Name (from its metadata). | 

## Example

```python
from chess_nnue_lab.models.stored_architecture import StoredArchitecture

# TODO update the JSON string below
json = "{}"
# create an instance of StoredArchitecture from a JSON string
stored_architecture_instance = StoredArchitecture.from_json(json)
# print the JSON string representation of the object
print(StoredArchitecture.to_json())

# convert the object into a dict
stored_architecture_dict = stored_architecture_instance.to_dict()
# create an instance of StoredArchitecture from a dict
stored_architecture_from_dict = StoredArchitecture.from_dict(stored_architecture_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


