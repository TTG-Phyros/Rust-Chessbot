# ArchitecturePage


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**items** | [**List[StoredArchitecture]**](StoredArchitecture.md) | Items of the page. | 
**next_cursor** | **str** | Cursor of the next page; absent on the last one. | [optional] 

## Example

```python
from chess_nnue_lab.models.architecture_page import ArchitecturePage

# TODO update the JSON string below
json = "{}"
# create an instance of ArchitecturePage from a JSON string
architecture_page_instance = ArchitecturePage.from_json(json)
# print the JSON string representation of the object
print(ArchitecturePage.to_json())

# convert the object into a dict
architecture_page_dict = architecture_page_instance.to_dict()
# create an instance of ArchitecturePage from a dict
architecture_page_from_dict = ArchitecturePage.from_dict(architecture_page_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


