# ModelPage


## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**items** | [**List[ModelView]**](ModelView.md) | Items of the page. | 
**next_cursor** | **str** | Cursor of the next page; absent on the last one. | [optional] 

## Example

```python
from chess_nnue_lab.models.model_page import ModelPage

# TODO update the JSON string below
json = "{}"
# create an instance of ModelPage from a JSON string
model_page_instance = ModelPage.from_json(json)
# print the JSON string representation of the object
print(ModelPage.to_json())

# convert the object into a dict
model_page_dict = model_page_instance.to_dict()
# create an instance of ModelPage from a dict
model_page_from_dict = ModelPage.from_dict(model_page_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


