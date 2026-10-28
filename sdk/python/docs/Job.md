# Job

A job.

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**attempt** | **int** | Times a worker took it (resumes count). | 
**created_at** | **int** | Creation time (Unix milliseconds). | 
**error** | **str** | Why it failed. | [optional] 
**finished_at** | **int** | When it reached a final status. | [optional] 
**id** | **UUID** | Identifier. | 
**model** | **UUID** | Model produced (training jobs that succeeded). | [optional] 
**name** | **str** | Name given at creation. | 
**progress** | [**Progress**](Progress.md) | Latest progress reported by its worker. | [optional] 
**spec** | [**JobSpec**](JobSpec.md) | What it does. | 
**started_at** | **int** | When a worker first took it. | [optional] 
**status** | [**JobStatus**](JobStatus.md) | Status. | 
**updated_at** | **int** | Last change. | 
**worker** | **str** | Worker holding it, or that held it last. | [optional] 

## Example

```python
from chess_nnue_lab.models.job import Job

# TODO update the JSON string below
json = "{}"
# create an instance of Job from a JSON string
job_instance = Job.from_json(json)
# print the JSON string representation of the object
print(Job.to_json())

# convert the object into a dict
job_dict = job_instance.to_dict()
# create an instance of Job from a dict
job_from_dict = Job.from_dict(job_dict)
```
[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


