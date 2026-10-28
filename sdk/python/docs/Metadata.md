# Metadata

Descriptive information about a network.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**author** | **str** | Who designed it. | [optional] 
**description** | **str** | What it is for. | [optional] 
**name** | **str** | Display name. | 
**tags** | **List[str]** | Lowercase labels: letters, digits and dashes. | [optional] 
**version** | **str** | Free-form version, e.g. &#x60;1.2.0&#x60;. | [optional] 

## Example

```python
from chess_nnue_lab.models.metadata import Metadata

# TODO update the JSON string below
json = "{}"
# create an instance of Metadata from a JSON string
metadata_instance = Metadata.from_json(json)
# print the JSON string representation of the object
print(Metadata.to_json())

# convert the object into a dict
metadata_dict = metadata_instance.to_dict()
# create an instance of Metadata from a dict
metadata_from_dict = Metadata.from_dict(metadata_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


