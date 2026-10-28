# JobSpecOneOf

See [`TrainingJob`].

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**architecture** | [**Architecture**](Architecture.md) |  | 
**architecture_hash** | **str** | Its hash. | 
**datasets** | [**List[DatasetRef]**](DatasetRef.md) | Training data. | 
**device** | [**Device**](Device.md) | Where it may run. | 
**hyperparameters** | [**Hyperparameters**](Hyperparameters.md) | Hyperparameters. | 
**seed** | **int** | Seed of the initialization and of the data order. | 
**trainer** | **str** | Trainer backend (&#x60;burn&#x60;). | 
**kind** | **str** |  | 

## Example

```python
from chess_nnue_lab.models.job_spec_one_of import JobSpecOneOf

# TODO update the JSON string below
json = "{}"
# create an instance of JobSpecOneOf from a JSON string
job_spec_one_of_instance = JobSpecOneOf.from_json(json)
# print the JSON string representation of the object
print(JobSpecOneOf.to_json())

# convert the object into a dict
job_spec_one_of_dict = job_spec_one_of_instance.to_dict()
# create an instance of JobSpecOneOf from a dict
job_spec_one_of_from_dict = JobSpecOneOf.from_dict(job_spec_one_of_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


