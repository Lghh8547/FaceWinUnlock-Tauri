use std::io::{Read, Write};

use base64::{engine::general_purpose, Engine};
use opencv::{
    core::{Mat, Point, Ptr, Rect, Scalar, Size, Vector},
    imgcodecs, imgproc,
    objdetect::{FaceDetectorYN, FaceRecognizerSF, FaceRecognizerSF_DisType},
    prelude::*,
};
use serde_json::json;
use tauri::Manager;
use crate::{utils::custom_result::CustomResult, AppState};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug)]
pub struct FaceDescriptor {
    pub name: String,
    pub feature: Vec<f32>,
}

impl FaceDescriptor {
    // 将 OpenCV 的 Mat 转换为可序列化的结构
    pub fn from_mat(name: &str, feature_mat: &Mat) -> Result<Self, Box<dyn std::error::Error>> {
        // 确保 Mat 是连续的，然后转换为 Vec
        let mut feature_vec: Vec<f32> = vec![0.0f32; feature_mat.total()];
        let data = feature_mat.data_typed::<f32>()?;
        feature_vec.copy_from_slice(data);
    
        Ok(FaceDescriptor {
            name: name.to_string(),
            feature: feature_vec,
        })
    }

    // 将特征向量还原回 OpenCV Mat
    pub fn to_mat(&self) -> Result<Mat, Box<dyn std::error::Error>> {
        // 从切片创建原始 Mat (默认为 N 行 1 列)
        let m = Mat::from_slice(&self.feature)?;
        
        // 变换形状为 1 行 128 列
        // reshape 返回的是 Result<BoxedRef<Mat>, ...>
        let m_reshaped = m.reshape(1, 1)?;
        
        // 使用 try_clone() 进行深拷贝，转回独立的 Mat 对象
        let final_mat = m_reshaped.try_clone()?;
        
        Ok(final_mat)
    }
}

struct CaptureResponse {
    display_base64: String, // 带框的
    raw_base64: String,     // 不带框的（仅缩放）
}

// 从图片中检测人脸
#[tauri::command]
pub async fn check_face_from_img(
    state: tauri::State<'_, AppState>,
    img_path: String,
) -> Result<CustomResult, CustomResult> {
    // 从fs读取图片
    // opencv不支持中文，搞了半个小时 ...
    let bytes = std::fs::read(&img_path)
        .map_err(|e| CustomResult::error(Some(format!("图片读取失败: {}", e)), None))?;
    let v = Vector::<u8>::from_iter(bytes);
    let src = imgcodecs::imdecode(&v, imgcodecs::IMREAD_COLOR)
        .map_err(|e| CustomResult::error(Some(format!("OpenCV 解码失败: {}", e)), None))?;

    if src.empty() {
        return Err(CustomResult::error(
            Some(String::from("图片读取失败")),
            None,
        ));
    }

    // 获取模型锁
    let mut d_lock = state.detector.write().await;
    let detector_wrapper = d_lock.as_mut();

    if detector_wrapper.is_none() {
        return Err(CustomResult::error(
            Some(String::from("请先初始化模型")),
            None,
        ));
    }
    let detector_wrapper = detector_wrapper.unwrap();
    let detector = &mut detector_wrapper.inner;

    let result = detect_and_format(detector, src)
        .map_err(|e| CustomResult::error(Some(format!("OpenCV 检测失败: {}", e)), None))?;

    Ok(CustomResult::success(
        None,
        Some(json!({
            "display_base64": result.display_base64,
            "raw_base64": result.raw_base64
        })),
    ))
}

// 从摄像头中检测人脸
#[tauri::command]
pub async fn check_face_from_camera(
    state: tauri::State<'_, AppState>,
) -> Result<CustomResult, CustomResult> {
    let frame = read_mat_from_camera(&state)
        .await
        .map_err(|e| CustomResult::error(Some(format!("摄像头读取失败: {}", e)), None))?;

    // 获取模型锁
    let mut d_lock = state.detector.write().await;
    let detector_wrapper = d_lock.as_mut();

    if detector_wrapper.is_none() {
        return Err(CustomResult::error(
            Some(String::from("请先初始化模型")),
            None,
        ));
    }
    let detector_wrapper = detector_wrapper.unwrap();

    let result = detect_and_format(&mut detector_wrapper.inner, frame)
        .map_err(|e| CustomResult::error(Some(format!("OpenCV 检测失败: {}", e)), None))?;

    Ok(CustomResult::success(
        None,
        Some(json!({
            "display_base64": result.display_base64,
            "raw_base64": result.raw_base64
        })),
    ))
}

// 一致性验证
#[tauri::command]
pub async fn verify_face(
    state: tauri::State<'_, AppState>,
    reference_base64: String,
) -> Result<CustomResult, CustomResult> {
    let frame = read_mat_from_camera(&state)
        .await
        .map_err(|e| CustomResult::error(Some(format!("摄像头读取失败: {}", e)), None))?;
    // 解码图片
    let ref_bytes = general_purpose::STANDARD
        .decode(reference_base64)
        .map_err(|e| CustomResult::error(Some(format!("图片解码失败: {}", e)), None))?;
    let v = Vector::<u8>::from_iter(ref_bytes);
    let ref_img = imgcodecs::imdecode(&v, opencv::imgcodecs::IMREAD_COLOR)
        .map_err(|e| CustomResult::error(Some(format!("从bse64读取图片失败: {}", e)), None))?;

    let mut d_lock = state.detector.write().await;
    let mut r_lock = state.recognizer.write().await;
    let detector = &mut d_lock
        .as_mut()
        .ok_or(CustomResult::error(
            Some(String::from("检测模型未初始化")),
            None,
        ))?
        .inner;
    let recognizer = &mut r_lock
        .as_mut()
        .ok_or(CustomResult::error(
            Some(String::from("识别模型未初始化")),
            None,
        ))?
        .inner;

    let ref_feature = get_feature(&ref_img, detector, recognizer)
        .map_err(|e| CustomResult::error(Some(format!("特征提取失败: {}", e)), None))?;
    let cur_feature = get_feature(&frame, detector, recognizer)
        .map_err(|e| CustomResult::error(Some(format!("特征提取失败: {}", e)), None))?;

    let score = recognizer
        .match_(
            &ref_feature,
            &cur_feature,
            FaceRecognizerSF_DisType::FR_COSINE.into(),
        )
        .map_err(|e| CustomResult::error(Some(format!("特征匹配失败: {}", e)), None))?;

    let mut result_mat = frame.clone();
    if let Ok(resize_mat) = resize_mat(&frame, 1270.0) {
        result_mat = resize_mat;
    }
    Ok(CustomResult::success(
        None,
        Some(json!(
            {
                "score": score,
                "display_base64": mat_to_base64(&result_mat)
            }
        )),
    ))
}

// 保存特征到文件
#[tauri::command]
pub async fn save_face_registration(
    handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    name: String,
    reference_base64: String,
) -> Result<CustomResult, CustomResult> {
    // 获取软件数据目录并创建 faces 文件夹
    let mut path = handle.path().resolve(
        "faces",
        tauri::path::BaseDirectory::Resource,
    ).map_err(|e| CustomResult::error(Some(format!("获取软件数据目录失败: {}", e)), None))?;

    if !path.exists() {
        std::fs::create_dir_all(&path).map_err(|e| CustomResult::error(Some(format!("创建 faces 文件夹失败: {}", e)), None))?;
    }

    // 解码图片
    let ref_bytes = general_purpose::STANDARD
        .decode(reference_base64)
        .map_err(|e| CustomResult::error(Some(format!("图片解码失败: {}", e)), None))?;
    let v = Vector::<u8>::from_iter(ref_bytes);
    let ref_img = imgcodecs::imdecode(&v, opencv::imgcodecs::IMREAD_COLOR)
        .map_err(|e| CustomResult::error(Some(format!("从bse64读取图片失败: {}", e)), None))?;

    let mut d_lock = state.detector.write().await;
    let mut r_lock = state.recognizer.write().await;
    let detector = &mut d_lock
        .as_mut()
        .ok_or(CustomResult::error(
            Some(String::from("检测模型未初始化")),
            None,
        ))?
        .inner;
    let recognizer = &mut r_lock
        .as_mut()
        .ok_or(CustomResult::error(
            Some(String::from("识别模型未初始化")),
            None,
        ))?
        .inner;

    let feature_mat = get_feature(&ref_img, detector, recognizer)
        .map_err(|e| CustomResult::error(Some(format!("特征提取失败: {}", e)), None))?;

    let descriptor = FaceDescriptor::from_mat(&name, &feature_mat)
        .map_err(|e| CustomResult::error(Some(format!("特征描述失败: {}", e)), None))?;

    let file_name = format!("{}.face", Uuid::new_v4());
    path.push(&file_name);

    save_face_data(&path, &descriptor).map_err(|e| CustomResult::error(Some(format!("保存特征数据失败: {}", e)), None))?;

    Ok(CustomResult::success(None, Some(json!({"file_name": file_name}))))
}

// 提取特征点
fn get_feature(
    img: &Mat,
    det: &mut Ptr<FaceDetectorYN>,
    rec: &mut Ptr<FaceRecognizerSF>,
) -> Result<Mat, String> {
    let mut faces = Mat::default();
    det.set_input_size(img.size().map_err(|e| format!("获取Mat尺寸失败: {}", e))?)
        .map_err(|e| format!("设置输入尺寸失败: {}", e))?;
    det.detect(img, &mut faces)
        .map_err(|e| format!("OpenCV 检测失败: {}", e))?;

    if faces.rows() > 0 {
        let mut aligned = Mat::default();
        let mut feature = Mat::default();

        // 人脸对齐与裁剪
        rec.align_crop(img, &faces.row(0).unwrap(), &mut aligned)
            .map_err(|e| format!("人脸对齐失败: {}", e))?;
        // 提取特征
        rec.feature(&aligned, &mut feature)
            .map_err(|e| format!("特征提取失败: {}", e))?;

        Ok(feature.clone())
    } else {
        Err("未检测到人脸".into())
    }
}

// 从摄像头中读取视频帧
async fn read_mat_from_camera(state: &tauri::State<'_, AppState>) -> Result<Mat, String> {
    let mut cam_lock = state.camera.write().await;
    // 如果摄像头没打开
    if cam_lock.is_none() {
        return Err(String::from("请先打开摄像头"));
    }

    let cam = cam_lock.as_mut().unwrap();
    let mut frame = Mat::default();

    cam.inner
        .read(&mut frame)
        .map_err(|e| format!("摄像头读取失败: {}", e))?;

    if frame.empty() {
        return Err(String::from("抓取到空帧"));
    }

    Ok(frame)
}

// 等比例缩放Mat
fn resize_mat(src: &Mat, max_dim: f32) -> Result<Mat, String> {
    let size = src.size().map_err(|e| e.to_string())?;
    let scale = (max_dim / (size.width.max(size.height) as f32)).min(1.0);

    let mut resize_mat = Mat::default();
    if scale < 1.0 {
        let new_size = Size::new(
            (size.width as f32 * scale) as i32,
            (size.height as f32 * scale) as i32,
        );
        imgproc::resize(&src, &mut resize_mat, new_size, 0.0, 0.0, imgproc::INTER_AREA).ok();
    } else {
        resize_mat = src.clone();
    }

    Ok(resize_mat)
}

// 处理人脸特征点
fn detect_and_format(
    detector: &mut Ptr<FaceDetectorYN>,
    src: Mat,
) -> Result<CaptureResponse, String> {
    // 等比例缩放
    let raw_mat = resize_mat(&src, 1270.0)?;

    // 检测
    let mut display_mat = raw_mat.clone(); // 用于显示的副本
    let mut faces = Mat::default();
    detector
        .set_input_size(
            display_mat
                .size()
                .map_err(|e| format!("获取Mat尺寸失败: {}", e))?,
        )
        .map_err(|e| format!("设置输入尺寸失败: {}", e))?;
    detector
        .detect(&display_mat, &mut faces)
        .map_err(|e| format!("OpenCV 检测失败: {}", e))?;

    if faces.rows() > 0 {
        let x = *faces
            .at_2d::<f32>(0, 0)
            .map_err(|e| format!("图片坐标获取失败: {}", e))?;
        let y = *faces
            .at_2d::<f32>(0, 1)
            .map_err(|e| format!("图片坐标获取失败: {}", e))?;
        let w = *faces
            .at_2d::<f32>(0, 2)
            .map_err(|e| format!("图片坐标获取失败: {}", e))?;
        let h = *faces
            .at_2d::<f32>(0, 3)
            .map_err(|e| format!("图片坐标获取失败: {}", e))?;

        let color = Scalar::new(255.0, 242.0, 0.0, 0.0);
        imgproc::rectangle(
            &mut display_mat,
            Rect::new(x as i32, y as i32, w as i32, h as i32),
            color,
            2,
            imgproc::LINE_8,
            0,
        )
        .map_err(|e| format!("图片绘制失败: {}", e))?;

        // 绘制五官
        for i in (4..14).step_by(2) {
            // 五官不影响检测结果，所以绘制失败可以忽略
            if let (Ok(px), Ok(py)) = (faces.at_2d::<f32>(0, i), faces.at_2d::<f32>(0, i + 1)) {
                imgproc::circle(
                    &mut display_mat,
                    Point::new(*px as i32, *py as i32),
                    4,
                    Scalar::new(0.0, 255.0, 0.0, 0.0), // 绿色
                    -1,
                    imgproc::LINE_AA,
                    0,
                )
                .ok();
            }
        }

        Ok(CaptureResponse {
            display_base64: mat_to_base64(&display_mat),
            raw_base64: mat_to_base64(&raw_mat),
        })
    } else {
        Err(String::from("未检测到人脸"))
    }
}

fn mat_to_base64(mat: &Mat) -> String {
    let mut buf = Vector::<u8>::new();
    imgcodecs::imencode(".jpg", mat, &mut buf, &Vector::new()).unwrap();
    format!(
        "data:image/jpeg;base64,{}",
        general_purpose::STANDARD.encode(buf.as_slice())
    )
}

// 保存人脸数据到文件
fn save_face_data(path: &std::path::PathBuf, data: &FaceDescriptor) -> Result<(), Box<dyn std::error::Error>> {
    let encoded: Vec<u8> = bincode::serialize(data)?;
    let mut file = std::fs::File::create(path)?;
    file.write_all(&encoded)?;
    Ok(())
}

// 从文件加载人脸数据
fn load_face_data(path: &str) -> Result<FaceDescriptor, Box<dyn std::error::Error>> {
    let mut file = std::fs::File::open(path)?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)?;
    let decoded: FaceDescriptor = bincode::deserialize(&buffer)?;
    Ok(decoded)
}